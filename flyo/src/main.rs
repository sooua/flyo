mod auth;
mod config;
mod fs_ops;
mod handlers;
mod ui;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, extract::FromRef};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::auth::{CurrentUser, SessionStore, build_session_cookie};
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub sessions: Arc<SessionStore>,
}

// Allow `State<Arc<SessionStore>>` etc. as extractors if we ever want them.
impl FromRef<AppState> for Arc<Config> {
    fn from_ref(input: &AppState) -> Self {
        input.cfg.clone()
    }
}
impl FromRef<AppState> for Arc<SessionStore> {
    fn from_ref(input: &AppState) -> Self {
        input.sessions.clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cfg = locate_config()?;
    tracing::info!(?cfg, "loaded configuration");

    let state = AppState {
        cfg: Arc::new(cfg.clone()),
        sessions: SessionStore::new(),
    };

    let app = Router::new()
        .route("/__health", get(|| async { "ok" }))
        .route("/__config", get(get_config))
        .route("/api/login", post(api_login))
        .route("/api/logout", post(api_logout))
        .route("/api/whoami", get(api_whoami))
        .route("/api/list", get(handlers::list))
        .route("/api/file", get(handlers::file))
        .route("/api/upload", post(handlers::upload))
        .route("/api/mkdir", post(handlers::mkdir))
        .route("/api/rename", post(handlers::rename))
        .route("/api/delete", post(handlers::delete))
        // Fallback: serve embedded UI (SPA). Must be last so /api/* wins.
        .fallback(ui::serve)
        .with_state(state);

    let listen = cfg
        .listen
        .first()
        .cloned()
        .unwrap_or_else(|| "0.0.0.0:9212".to_string());
    let addr: SocketAddr = listen.parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("flyo listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

fn locate_config() -> anyhow::Result<Config> {
    let candidates = [
        PathBuf::from("flyo.conf"),
        PathBuf::from("webd.conf"),
        PathBuf::from("/etc/flyo.conf"),
        PathBuf::from("/etc/webd.conf"),
    ];
    for path in &candidates {
        if path.exists() {
            tracing::info!("loading config from {}", path.display());
            return Config::load(path);
        }
    }
    tracing::warn!("no config file found; using defaults");
    Ok(Config {
        root: std::env::current_dir()?,
        listen: vec!["0.0.0.0:9212".to_string()],
        ..Default::default()
    })
}

// ----- handlers -----

async fn get_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::to_value(&*state.cfg).unwrap())
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    user: String,
    pass: String,
}

#[derive(Debug, Serialize)]
struct WhoAmI {
    authenticated: bool,
    user: Option<String>,
    perms: crate::config::Perms,
}

async fn api_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Response {
    let Some(u) = state
        .cfg
        .users
        .iter()
        .find(|u| u.name == body.user && u.pass == body.pass)
    else {
        tracing::info!(user = %body.user, "login rejected");
        return (StatusCode::UNAUTHORIZED, "invalid credentials").into_response();
    };

    let token = state.sessions.create(&u.name);
    let cookie = build_session_cookie(Some(&token));

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&cookie).unwrap(),
    );

    let body = WhoAmI {
        authenticated: true,
        user: Some(u.name.clone()),
        perms: u.perms,
    };
    tracing::info!(user = %u.name, "login ok");
    (StatusCode::OK, headers, Json(body)).into_response()
}

async fn api_logout(State(state): State<AppState>, user: CurrentUser) -> Response {
    if let Some(name) = user.user.as_deref() {
        state.sessions.drop_by_user(name);
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&build_session_cookie(None)).unwrap(),
    );
    (StatusCode::OK, headers, "ok").into_response()
}

async fn api_whoami(user: CurrentUser) -> Json<WhoAmI> {
    Json(WhoAmI {
        authenticated: user.user.is_some(),
        user: user.user,
        perms: user.perms,
    })
}
