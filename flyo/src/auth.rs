//! Session-based authentication.
//!
//! Sessions are stored in-memory; restart of the server invalidates all
//! sessions. The chosen cookie is `flyo_sid`, HttpOnly + SameSite=Lax.
//!
//! Effective permissions for a request come from one of two places:
//!   - if a valid session cookie is present → the matching user's perms
//!   - otherwise                            → the guest's perms

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use rand::Rng;
use serde::Serialize;

use crate::AppState;
use crate::config::{Config, Perms};

pub const SESSION_COOKIE: &str = "flyo_sid";

/// In-memory session table: random token → username.
#[derive(Debug, Default)]
pub struct SessionStore {
    inner: RwLock<HashMap<String, String>>,
}

impl SessionStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(HashMap::new()),
        })
    }

    pub fn create(&self, username: &str) -> String {
        let token = gen_token();
        self.inner
            .write()
            .unwrap()
            .insert(token.clone(), username.to_string());
        token
    }

    pub fn resolve(&self, token: &str) -> Option<String> {
        self.inner.read().unwrap().get(token).cloned()
    }

    pub fn evict(&self, token: &str) {
        self.inner.write().unwrap().remove(token);
    }

    /// Invalidate every session belonging to a given user — used on logout.
    pub fn drop_by_user(&self, username: &str) {
        self.inner
            .write()
            .unwrap()
            .retain(|_token, name| name != username);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }
}

/// Random 32-char alphanumeric token (~190 bits of entropy).
fn gen_token() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..32)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Extractor giving the request handler the effective identity + permissions.
///
/// - `user`  is `Some(name)` for an authenticated request, `None` for guest.
/// - `perms` is always populated.
#[derive(Debug, Clone, Serialize)]
pub struct CurrentUser {
    pub user: Option<String>,
    pub perms: Perms,
}

impl CurrentUser {
    /// Compute effective identity from a (possibly missing) session token.
    pub fn from_token(cfg: &Config, sessions: &SessionStore, token: Option<&str>) -> Self {
        if let Some(t) = token {
            if let Some(name) = sessions.resolve(t) {
                if let Some(u) = cfg.users.iter().find(|u| u.name == name) {
                    return Self {
                        user: Some(u.name.clone()),
                        perms: u.perms,
                    };
                }
                // Session token referenced a user that was removed from config.
                // Treat as guest.
            }
        }
        Self {
            user: None,
            perms: cfg.guest,
        }
    }
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_session_cookie(parts);
        Ok(CurrentUser::from_token(
            &state.cfg,
            &state.sessions,
            token.as_deref(),
        ))
    }
}

/// Pull `flyo_sid=...` from the `Cookie` header(s), if present.
fn extract_session_cookie(parts: &Parts) -> Option<String> {
    for value in parts.headers.get_all(header::COOKIE) {
        let Ok(text) = value.to_str() else { continue };
        for pair in text.split(';') {
            let pair = pair.trim();
            if let Some(v) = pair.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Build the `Set-Cookie` header value for granting / revoking the session.
pub fn build_session_cookie(token: Option<&str>) -> String {
    match token {
        Some(t) => format!("{SESSION_COOKIE}={t}; Path=/; HttpOnly; SameSite=Lax"),
        None => format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::User;

    fn cfg_with_users() -> Config {
        Config {
            users: vec![
                User {
                    name: "alice".into(),
                    pass: "p1".into(),
                    perms: Perms {
                        access: true,
                        list: true,
                        upload: true,
                        modify: true,
                        ..Default::default()
                    },
                },
                User {
                    name: "bob".into(),
                    pass: "p2".into(),
                    perms: Perms {
                        access: true,
                        list: true,
                        ..Default::default()
                    },
                },
            ],
            ..Config::default()
        }
    }

    #[test]
    fn token_is_32_chars_alnum() {
        let t = gen_token();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric()));
        // Two consecutive tokens should not collide.
        assert_ne!(t, gen_token());
    }

    #[test]
    fn session_lifecycle() {
        let store = SessionStore::new();
        assert_eq!(store.len(), 0);
        let t = store.create("alice");
        assert_eq!(store.len(), 1);
        assert_eq!(store.resolve(&t).as_deref(), Some("alice"));
        store.evict(&t);
        assert_eq!(store.len(), 0);
        assert!(store.resolve(&t).is_none());
    }

    #[test]
    fn drop_by_user_clears_all_their_sessions() {
        let store = SessionStore::new();
        let _t1 = store.create("alice");
        let _t2 = store.create("alice"); // multiple devices
        let t3 = store.create("bob");
        assert_eq!(store.len(), 3);
        store.drop_by_user("alice");
        assert_eq!(store.len(), 1);
        assert_eq!(store.resolve(&t3).as_deref(), Some("bob"));
    }

    #[test]
    fn no_cookie_yields_guest() {
        let cfg = cfg_with_users();
        let store = SessionStore::new();
        let cu = CurrentUser::from_token(&cfg, &store, None);
        assert!(cu.user.is_none());
        // Default Config::default uses Perms::guest_default() → r+l
        assert!(cu.perms.access && cu.perms.list);
        assert!(!cu.perms.upload);
    }

    #[test]
    fn valid_session_yields_user_perms() {
        let cfg = cfg_with_users();
        let store = SessionStore::new();
        let t = store.create("alice");
        let cu = CurrentUser::from_token(&cfg, &store, Some(&t));
        assert_eq!(cu.user.as_deref(), Some("alice"));
        assert!(cu.perms.upload);
        assert!(cu.perms.modify);
    }

    #[test]
    fn session_for_removed_user_falls_back_to_guest() {
        let mut cfg = cfg_with_users();
        let store = SessionStore::new();
        let t = store.create("alice");
        // Simulate config reload that drops alice.
        cfg.users.retain(|u| u.name != "alice");
        let cu = CurrentUser::from_token(&cfg, &store, Some(&t));
        assert!(cu.user.is_none(), "stale session must not impersonate");
    }

    #[test]
    fn build_cookie_set_and_clear() {
        let set = build_session_cookie(Some("abc"));
        assert!(set.contains("flyo_sid=abc"));
        assert!(set.contains("HttpOnly"));
        assert!(set.contains("SameSite=Lax"));
        let clear = build_session_cookie(None);
        assert!(clear.contains("Max-Age=0"));
    }
}
