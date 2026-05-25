//! Request gatekeeping: IP allow/block + per-IP rate limiting.
//!
//! Allow / block semantics:
//!   - If any `block` rule matches → deny.
//!   - If `allow` rules are present, the request IP must match at least one of
//!     them; otherwise → deny. (Empty allow list = allow all.)
//!
//! Rate limiting is a fixed-window counter per IP. Simpler than a token bucket
//! and sufficient at this layer — the downstream flyo enforces real auth.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::{IpRule, RateLimit};

#[derive(Debug)]
pub struct Guard {
    pub allow: Vec<IpRule>,
    pub block: Vec<IpRule>,
    pub rate: Option<RateLimit>,
    counters: Mutex<HashMap<IpAddr, Counter>>,
}

#[derive(Debug, Clone, Copy)]
struct Counter {
    window_start: Instant,
    count: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    BlockedByList,
    BlockedByAllowList,
    RateLimited { retry_after: Duration },
}

impl Guard {
    pub fn new(allow: Vec<IpRule>, block: Vec<IpRule>, rate: Option<RateLimit>) -> Self {
        Self {
            allow,
            block,
            rate,
            counters: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, addr: IpAddr) -> Decision {
        if self.block.iter().any(|r| r.matches(addr)) {
            return Decision::BlockedByList;
        }
        if !self.allow.is_empty() && !self.allow.iter().any(|r| r.matches(addr)) {
            return Decision::BlockedByAllowList;
        }
        if let Some(rate) = self.rate {
            let now = Instant::now();
            let mut map = self.counters.lock().unwrap();

            // Opportunistic GC — if the map grows large, drop expired entries.
            if map.len() > 4096 {
                map.retain(|_, c| now.duration_since(c.window_start) < rate.window);
            }

            let entry = map.entry(addr).or_insert(Counter {
                window_start: now,
                count: 0,
            });
            if now.duration_since(entry.window_start) >= rate.window {
                entry.window_start = now;
                entry.count = 0;
            }
            entry.count += 1;
            if entry.count > rate.requests {
                let retry_after = rate
                    .window
                    .saturating_sub(now.duration_since(entry.window_start));
                return Decision::RateLimited { retry_after };
            }
        }
        Decision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn rule(s: &str) -> IpRule {
        IpRule {
            raw: s.to_string(),
            net: Some(s.parse().unwrap_or_else(|_| ipnet::IpNet::from(IpAddr::from_str(s).unwrap()))),
        }
    }

    #[test]
    fn block_list_denies() {
        let g = Guard::new(vec![], vec![rule("1.2.3.4")], None);
        assert_eq!(g.check("1.2.3.4".parse().unwrap()), Decision::BlockedByList);
        assert_eq!(g.check("1.2.3.5".parse().unwrap()), Decision::Allow);
    }

    #[test]
    fn allow_list_denies_others() {
        let g = Guard::new(vec![rule("10.0.0.0/8")], vec![], None);
        assert_eq!(g.check("10.5.5.5".parse().unwrap()), Decision::Allow);
        assert_eq!(g.check("11.0.0.1".parse().unwrap()), Decision::BlockedByAllowList);
    }

    #[test]
    fn block_beats_allow() {
        let g = Guard::new(vec![rule("10.0.0.0/8")], vec![rule("10.0.0.42")], None);
        assert_eq!(g.check("10.0.0.42".parse().unwrap()), Decision::BlockedByList);
    }

    #[test]
    fn rate_limit_enforces_window() {
        let g = Guard::new(
            vec![],
            vec![],
            Some(RateLimit { requests: 3, window: Duration::from_secs(60) }),
        );
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(g.check(ip), Decision::Allow);
        assert_eq!(g.check(ip), Decision::Allow);
        assert_eq!(g.check(ip), Decision::Allow);
        assert!(matches!(g.check(ip), Decision::RateLimited { .. }));
    }

    #[test]
    fn rate_limit_is_per_ip() {
        let g = Guard::new(
            vec![],
            vec![],
            Some(RateLimit { requests: 1, window: Duration::from_secs(60) }),
        );
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        let b: IpAddr = "10.0.0.2".parse().unwrap();
        assert_eq!(g.check(a), Decision::Allow);
        assert!(matches!(g.check(a), Decision::RateLimited { .. }));
        assert_eq!(g.check(b), Decision::Allow);
    }

    #[test]
    fn empty_allow_means_anyone() {
        let g = Guard::new(vec![], vec![], None);
        assert_eq!(g.check("8.8.8.8".parse().unwrap()), Decision::Allow);
    }
}
