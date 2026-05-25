//! flyo-proxy.conf parser.
//!
//! Directives (case-insensitive):
//!   Proxy.Listen      <addr:port>          (default 0.0.0.0:8443)
//!   Proxy.Upstream    http://host:port     (default http://127.0.0.1:9212)
//!   Proxy.Cert        <path>               (PEM-encoded chain)
//!   Proxy.Key         <path>               (PEM-encoded key)
//!   Proxy.SelfSigned                       (auto-generate dev cert at startup)
//!   Proxy.Allow       <ip-or-cidr>         (repeatable)
//!   Proxy.Block       <ip-or-cidr>         (repeatable)
//!   Proxy.RateLimit   <reqs>/<window>      (e.g. 100/min)
//!   Proxy.LogLevel    info|debug|warn|error

use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen: String,
    pub upstream: String,
    pub tls: TlsMode,
    pub allow: Vec<IpRule>,
    pub block: Vec<IpRule>,
    pub rate_limit: Option<RateLimit>,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TlsMode {
    /// Plain HTTP — no TLS at all. Useful for local development.
    Plain,
    /// Auto-generate a self-signed cert in-memory at startup.
    SelfSigned { dns_names: Vec<String> },
    /// Read PEM cert + key from disk.
    Files { cert: PathBuf, key: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpRule {
    pub raw: String,
    #[serde(skip)]
    pub net: Option<IpNet>,
}

impl IpRule {
    pub fn matches(&self, addr: IpAddr) -> bool {
        match &self.net {
            Some(net) => net.contains(&addr),
            None => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests: u32,
    pub window: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:8443".to_string(),
            upstream: "http://127.0.0.1:9212".to_string(),
            tls: TlsMode::Plain,
            allow: Vec::new(),
            block: Vec::new(),
            rate_limit: None,
            log_level: "info".to_string(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut cfg = Config::default();
        let mut cert: Option<PathBuf> = None;
        let mut key: Option<PathBuf> = None;
        let mut self_signed_names: Option<Vec<String>> = None;

        for (lineno, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let tokens = tokenize(line);
            if tokens.is_empty() {
                continue;
            }
            let key_lc = tokens[0].to_ascii_lowercase();
            let args = &tokens[1..];

            match key_lc.as_str() {
                "proxy.listen" => {
                    if args.is_empty() {
                        bail!("line {}: Proxy.Listen requires an address", lineno + 1);
                    }
                    cfg.listen = normalize_listen(&args[0]);
                }
                "proxy.upstream" => {
                    if args.is_empty() {
                        bail!("line {}: Proxy.Upstream requires a URL", lineno + 1);
                    }
                    cfg.upstream = args[0].clone();
                }
                "proxy.cert" => {
                    cert = Some(PathBuf::from(&args[0]));
                }
                "proxy.key" => {
                    key = Some(PathBuf::from(&args[0]));
                }
                "proxy.selfsigned" => {
                    // Names default to localhost if none given.
                    let names: Vec<String> = if args.is_empty() {
                        vec!["localhost".to_string()]
                    } else {
                        args.to_vec()
                    };
                    self_signed_names = Some(names);
                }
                "proxy.allow" => {
                    cfg.allow.push(parse_rule(&args[0], lineno + 1)?);
                }
                "proxy.block" => {
                    cfg.block.push(parse_rule(&args[0], lineno + 1)?);
                }
                "proxy.ratelimit" => {
                    cfg.rate_limit = Some(parse_rate(&args[0], lineno + 1)?);
                }
                "proxy.loglevel" => {
                    cfg.log_level = args[0].to_ascii_lowercase();
                }
                _ => tracing::warn!("line {}: unknown directive '{}'", lineno + 1, tokens[0]),
            }
        }

        // Resolve TLS mode after all directives are seen.
        cfg.tls = match (cert, key, self_signed_names) {
            (Some(c), Some(k), _) => TlsMode::Files { cert: c, key: k },
            (_, _, Some(names)) => TlsMode::SelfSigned { dns_names: names },
            _ => TlsMode::Plain,
        };

        Ok(cfg)
    }
}

fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn normalize_listen(raw: &str) -> String {
    if raw.contains(':') {
        raw.to_string()
    } else if raw.chars().all(|c| c.is_ascii_digit()) {
        format!("0.0.0.0:{raw}")
    } else {
        raw.to_string()
    }
}

fn parse_rule(raw: &str, lineno: usize) -> Result<IpRule> {
    // Try CIDR first (e.g. "10.0.0.0/8"), then a bare IP.
    let net = if raw.contains('/') {
        Some(raw.parse::<IpNet>().with_context(|| {
            format!("line {lineno}: invalid CIDR '{raw}'")
        })?)
    } else {
        let ip: IpAddr = raw.parse().with_context(|| {
            format!("line {lineno}: invalid IP address '{raw}'")
        })?;
        Some(IpNet::from(ip))
    };
    Ok(IpRule {
        raw: raw.to_string(),
        net,
    })
}

fn parse_rate(raw: &str, lineno: usize) -> Result<RateLimit> {
    let (n, unit) = raw
        .split_once('/')
        .with_context(|| format!("line {lineno}: rate limit must be N/unit, got '{raw}'"))?;
    let requests: u32 = n
        .trim()
        .parse()
        .with_context(|| format!("line {lineno}: invalid request count '{n}'"))?;
    let window = match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "second" => Duration::from_secs(1),
        "m" | "min" | "minute" => Duration::from_secs(60),
        "h" | "hour" => Duration::from_secs(3600),
        other => bail!("line {lineno}: unknown rate window '{other}'"),
    };
    Ok(RateLimit { requests, window })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config() {
        let cfg = Config::parse("Proxy.Listen 8443\nProxy.Upstream http://127.0.0.1:9212\n").unwrap();
        assert_eq!(cfg.listen, "0.0.0.0:8443");
        assert_eq!(cfg.upstream, "http://127.0.0.1:9212");
        assert!(matches!(cfg.tls, TlsMode::Plain));
    }

    #[test]
    fn cert_files_take_precedence_over_self_signed() {
        let text = r#"
            Proxy.SelfSigned myhost
            Proxy.Cert /tmp/c.pem
            Proxy.Key /tmp/k.pem
        "#;
        let cfg = Config::parse(text).unwrap();
        assert!(matches!(cfg.tls, TlsMode::Files { .. }));
    }

    #[test]
    fn allow_block_parses_cidr_and_bare_ip() {
        let text = "Proxy.Allow 10.0.0.0/8\nProxy.Block 1.2.3.4\n";
        let cfg = Config::parse(text).unwrap();
        assert_eq!(cfg.allow.len(), 1);
        assert!(cfg.allow[0].matches("10.5.5.5".parse().unwrap()));
        assert!(!cfg.allow[0].matches("11.0.0.1".parse().unwrap()));
        assert!(cfg.block[0].matches("1.2.3.4".parse().unwrap()));
    }

    #[test]
    fn rate_limit_units() {
        assert_eq!(
            Config::parse("Proxy.RateLimit 100/min").unwrap().rate_limit.unwrap().window,
            Duration::from_secs(60)
        );
        assert_eq!(
            Config::parse("Proxy.RateLimit 10/sec").unwrap().rate_limit.unwrap().window,
            Duration::from_secs(1)
        );
        assert_eq!(
            Config::parse("Proxy.RateLimit 5000/hour").unwrap().rate_limit.unwrap().window,
            Duration::from_secs(3600)
        );
    }

    #[test]
    fn unknown_unit_errors() {
        assert!(Config::parse("Proxy.RateLimit 1/year").is_err());
    }
}
