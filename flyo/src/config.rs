//! webd.conf compatible configuration parser.
//!
//! Recognised directives (case-insensitive):
//!   Webd.Root     <path>
//!   Webd.Listen   <addr-or-port>          (repeatable)
//!   Webd.User     <perm_tag> <user> <pass> (repeatable, max 3 users)
//!   Webd.Guest    <perm_tag>
//!   Webd.Hide                              (no value, Windows tray-only)
//!   Webd.Browser  <path>
//!
//! Lines starting with `#` are comments. Paths may be double-quoted.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub root: PathBuf,
    pub listen: Vec<String>,
    pub users: Vec<User>,
    pub guest: Perms,
    pub hide_tray: bool,
    pub browser: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            listen: Vec::new(),
            users: Vec::new(),
            // Traditional webd default: guest can list and access (read).
            guest: Perms::guest_default(),
            hide_tray: false,
            browser: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub pass: String,
    pub perms: Perms,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Perms {
    pub access: bool,         // r
    pub list: bool,           // l
    pub upload: bool,         // u
    pub modify: bool,         // m (delete/move/rename)
    pub show_hidden: bool,    // S
    pub play_media: bool,     // T
    pub force_download: bool, // D
}

impl Perms {
    pub fn from_tag(tag: &str) -> Self {
        let mut p = Perms::default();
        if tag == "0" {
            return p; // explicit "no permissions"
        }
        for ch in tag.chars() {
            match ch {
                'r' => p.access = true,
                'l' => p.list = true,
                'u' => p.upload = true,
                'm' => p.modify = true,
                'S' => p.show_hidden = true,
                'T' => p.play_media = true,
                'D' => p.force_download = true,
                _ => {} // silently ignore unknown tags (forward-compat)
            }
        }
        p
    }

    pub fn to_tag(self) -> String {
        let mut s = String::new();
        if self.access { s.push('r'); }
        if self.list { s.push('l'); }
        if self.upload { s.push('u'); }
        if self.modify { s.push('m'); }
        if self.show_hidden { s.push('S'); }
        if self.play_media { s.push('T'); }
        if self.force_download { s.push('D'); }
        if s.is_empty() { s.push('0'); }
        s
    }

    /// Traditional webd guest default: can list and read.
    pub const fn guest_default() -> Self {
        Self {
            access: true,
            list: true,
            upload: false,
            modify: false,
            show_hidden: false,
            play_media: false,
            force_download: false,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        // Config::default() already sets guest to Perms::guest_default() (r+l).
        let mut cfg = Config::default();

        for (lineno, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let tokens = tokenize(line);
            if tokens.is_empty() {
                continue;
            }
            let key = tokens[0].to_ascii_lowercase();
            let args = &tokens[1..];

            match key.as_str() {
                "webd.root" => {
                    if args.is_empty() {
                        bail!("line {}: Webd.Root requires a path", lineno + 1);
                    }
                    cfg.root = PathBuf::from(&args[0]);
                }
                "webd.listen" => {
                    if args.is_empty() {
                        bail!("line {}: Webd.Listen requires an address or port", lineno + 1);
                    }
                    cfg.listen.push(normalize_listen(&args[0]));
                }
                "webd.user" => {
                    if args.len() < 3 {
                        bail!(
                            "line {}: Webd.User requires <perm_tag> <user> <pass>",
                            lineno + 1
                        );
                    }
                    if cfg.users.len() >= 3 {
                        // Original webd caps at 3 users; we warn but keep going.
                        tracing::warn!("line {}: ignoring extra Webd.User beyond 3", lineno + 1);
                        continue;
                    }
                    cfg.users.push(User {
                        perms: Perms::from_tag(&args[0]),
                        name: args[1].clone(),
                        pass: args[2].clone(),
                    });
                }
                "webd.guest" => {
                    if args.is_empty() {
                        bail!("line {}: Webd.Guest requires a perm tag", lineno + 1);
                    }
                    cfg.guest = Perms::from_tag(&args[0]);
                }
                "webd.hide" => {
                    cfg.hide_tray = true;
                }
                "webd.browser" => {
                    cfg.browser = args.first().cloned();
                }
                _ => {
                    tracing::warn!("line {}: unknown directive '{}'", lineno + 1, tokens[0]);
                }
            }
        }

        if cfg.listen.is_empty() {
            cfg.listen.push("0.0.0.0:9212".to_string());
        }

        Ok(cfg)
    }
}

/// Tokenize a single line, supporting double-quoted strings for paths with
/// spaces (matches original webd's `Webd.Root "D:\my share"` syntax).
fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Accept the same listen forms as original webd:
///   "9212"             -> "0.0.0.0:9212"
///   "10.0.0.1:9212"    -> as-is
///   "[::]:9212"        -> as-is
fn normalize_listen(raw: &str) -> String {
    if raw.contains(':') {
        raw.to_string()
    } else if raw.chars().all(|c| c.is_ascii_digit()) {
        format!("0.0.0.0:{raw}")
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_config() {
        let text = r#"
            # comment
            Webd.Root /mnt/sda1
            Webd.Listen 9212
            Webd.Listen [::]:9212
            Webd.User rlumS user1 pass1
            Webd.User rl user2 pass2
            Webd.Guest rl
            Webd.Hide
        "#;
        let cfg = Config::parse(text).unwrap();
        assert_eq!(cfg.root, PathBuf::from("/mnt/sda1"));
        assert_eq!(cfg.listen.len(), 2);
        assert_eq!(cfg.listen[0], "0.0.0.0:9212");
        assert_eq!(cfg.listen[1], "[::]:9212");
        assert_eq!(cfg.users.len(), 2);
        assert_eq!(cfg.users[0].name, "user1");
        assert!(cfg.users[0].perms.modify);
        assert!(cfg.users[0].perms.show_hidden);
        assert!(!cfg.users[1].perms.modify);
        assert!(cfg.hide_tray);
    }

    #[test]
    fn quoted_path_with_spaces() {
        let text = r#"Webd.Root "D:\my share""#;
        let cfg = Config::parse(text).unwrap();
        assert_eq!(cfg.root, PathBuf::from(r"D:\my share"));
    }

    #[test]
    fn guest_zero_disables_all() {
        let cfg = Config::parse("Webd.Guest 0").unwrap();
        assert!(!cfg.guest.access && !cfg.guest.list);
    }

    #[test]
    fn perms_roundtrip() {
        let p = Perms::from_tag("rlumS");
        assert_eq!(p.to_tag(), "rlumS");
        let p = Perms::from_tag("0");
        assert_eq!(p.to_tag(), "0");
    }

    #[test]
    fn caps_users_at_three_with_warning() {
        let text = "
            Webd.User r u1 p1
            Webd.User r u2 p2
            Webd.User r u3 p3
            Webd.User r u4 p4
        ";
        let cfg = Config::parse(text).unwrap();
        assert_eq!(cfg.users.len(), 3);
    }
}
