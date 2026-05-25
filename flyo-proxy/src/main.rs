//! flyo-proxy — an optional reverse proxy in front of flyo.
//!
//! Architecture is intentionally small: one async TCP accept loop, one per-conn
//! task, each connection terminated as TLS (if configured) and then driven
//! through `hyper::server::conn::http1`. Every request gets routed to a single
//! handler that either:
//!   1. denies the request (IP blocked / rate-limited), or
//!   2. forwards it to the upstream flyo, streaming the body in both directions.

mod config;
mod guard;
mod tls;

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{HeaderName, HeaderValue, HOST};
use hyper::{Request, Response, StatusCode, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as ServerBuilder;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing_subscriber::EnvFilter;

use crate::config::{Config, TlsMode};
use crate::guard::{Decision, Guard};

const PROXY_NAME: &str = concat!("flyo-proxy/", env!("CARGO_PKG_VERSION"));

/// Hop-by-hop headers, per RFC 7230 §6.1. These must not be forwarded.
const HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

struct AppState {
    cfg: Config,
    guard: Arc<Guard>,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Incoming>,
    upstream: hyper::Uri,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = locate_config()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cfg.log_level)),
        )
        .init();

    tracing::info!(?cfg, "flyo-proxy starting");

    let upstream: Uri = cfg
        .upstream
        .parse()
        .with_context(|| format!("invalid upstream URL: {}", cfg.upstream))?;

    if upstream.scheme_str() != Some("http") {
        anyhow::bail!(
            "Proxy.Upstream must be http://… (TLS-to-upstream is not implemented yet); got {}",
            cfg.upstream
        );
    }

    let guard = Arc::new(Guard::new(
        cfg.allow.clone(),
        cfg.block.clone(),
        cfg.rate_limit,
    ));

    // Install crypto provider for rustls. ring is the default — explicit so
    // it's clear what we're doing.
    if matches!(cfg.tls, TlsMode::SelfSigned { .. } | TlsMode::Files { .. }) {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let tls_cfg = tls::build_server_config(&cfg.tls)?;
    let acceptor = tls_cfg.map(TlsAcceptor::from);

    let addr: SocketAddr = cfg
        .listen
        .parse()
        .with_context(|| format!("invalid Proxy.Listen address: {}", cfg.listen))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    let client = Client::builder(TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(30))
        .build_http::<Incoming>();

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        guard,
        client,
        upstream,
    });

    let scheme = if acceptor.is_some() { "https" } else { "http" };
    tracing::info!("flyo-proxy listening on {scheme}://{addr} → {}", cfg.upstream);

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!("accept error: {e}");
                continue;
            }
        };
        let state = state.clone();
        let acceptor = acceptor.clone();

        tokio::spawn(async move {
            if let Err(e) = serve_conn(stream, peer, acceptor, state).await {
                tracing::debug!(?peer, "conn closed with error: {e:#}");
            }
        });
    }
}

async fn serve_conn(
    stream: tokio::net::TcpStream,
    peer: SocketAddr,
    acceptor: Option<TlsAcceptor>,
    state: Arc<AppState>,
) -> Result<()> {
    let svc = hyper::service::service_fn(move |req| {
        let state = state.clone();
        async move { Ok::<_, Infallible>(handle(state, peer.ip(), req).await) }
    });

    let builder = ServerBuilder::new(TokioExecutor::new());

    match acceptor {
        Some(acceptor) => {
            let tls = acceptor.accept(stream).await?;
            builder
                .serve_connection(TokioIo::new(tls), svc)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        None => {
            builder
                .serve_connection(TokioIo::new(stream), svc)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }
    Ok(())
}

async fn handle(
    state: Arc<AppState>,
    peer: IpAddr,
    mut req: Request<Incoming>,
) -> Response<Full<Bytes>> {
    // Gate: IP and rate.
    match state.guard.check(peer) {
        Decision::Allow => {}
        Decision::BlockedByList | Decision::BlockedByAllowList => {
            tracing::info!(%peer, path = %req.uri().path(), "blocked by IP rule");
            return text_response(StatusCode::FORBIDDEN, "blocked by proxy IP policy");
        }
        Decision::RateLimited { retry_after } => {
            tracing::info!(%peer, retry = ?retry_after, "rate limited");
            let mut r = text_response(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded");
            r.headers_mut().insert(
                hyper::header::RETRY_AFTER,
                HeaderValue::from_str(&retry_after.as_secs().max(1).to_string()).unwrap(),
            );
            return r;
        }
    }

    let started = std::time::Instant::now();
    let method = req.method().clone();
    let path_and_q = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    // Rewrite URI to point at upstream.
    let upstream_uri = match build_upstream_uri(&state.upstream, &path_and_q) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = ?e, "upstream URI build failed");
            return text_response(StatusCode::BAD_GATEWAY, "bad upstream uri");
        }
    };
    *req.uri_mut() = upstream_uri;

    // Inject X-Forwarded-* headers, replace Host.
    strip_hop_headers(req.headers_mut());
    let xff_value = HeaderValue::from_str(&peer.to_string()).unwrap();
    req.headers_mut()
        .insert(HeaderName::from_static("x-forwarded-for"), xff_value.clone());
    req.headers_mut()
        .insert(HeaderName::from_static("x-real-ip"), xff_value);
    req.headers_mut().insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static(if state.has_tls() { "https" } else { "http" }),
    );
    if let Some(host) = state.upstream.host() {
        if let Ok(h) = HeaderValue::from_str(host) {
            req.headers_mut().insert(HOST, h);
        }
    }

    // Forward and stream the response.
    let upstream_res = match state.client.request(req).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "upstream request failed");
            return text_response(StatusCode::BAD_GATEWAY, "upstream unreachable");
        }
    };

    let status = upstream_res.status();
    let (parts, body) = upstream_res.into_parts();
    let bytes = match body.collect().await {
        Ok(c) => c.to_bytes(),
        Err(e) => {
            tracing::warn!(error = %e, "reading upstream body failed");
            return text_response(StatusCode::BAD_GATEWAY, "upstream body error");
        }
    };

    let mut resp = Response::from_parts(parts, Full::new(bytes));
    strip_hop_headers(resp.headers_mut());
    inject_security_headers(resp.headers_mut(), state.has_tls());
    *resp.status_mut() = status;

    tracing::info!(
        %peer,
        %method,
        path = %path_and_q,
        status = status.as_u16(),
        ms = started.elapsed().as_millis() as u64,
        "request"
    );

    resp
}

impl AppState {
    fn has_tls(&self) -> bool {
        !matches!(self.cfg.tls, TlsMode::Plain)
    }
}

fn build_upstream_uri(base: &Uri, path_and_q: &str) -> Result<Uri> {
    let scheme = base.scheme_str().unwrap_or("http");
    let authority = base
        .authority()
        .map(|a| a.as_str().to_string())
        .ok_or_else(|| anyhow::anyhow!("upstream missing authority"))?;
    let uri = format!("{scheme}://{authority}{path_and_q}");
    Ok(uri.parse()?)
}

fn strip_hop_headers(h: &mut hyper::HeaderMap) {
    for name in HOP_HEADERS {
        h.remove(*name);
    }
}

fn inject_security_headers(h: &mut hyper::HeaderMap, https: bool) {
    h.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    h.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    h.insert(
        HeaderName::from_static("server"),
        HeaderValue::from_static(PROXY_NAME),
    );
    if https {
        // HSTS only makes sense when we're already terminating HTTPS.
        h.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
}

fn text_response(status: StatusCode, msg: &'static str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(msg)))
        .unwrap()
}

fn locate_config() -> Result<Config> {
    let candidates = [
        PathBuf::from("flyo-proxy.conf"),
        PathBuf::from("/etc/flyo-proxy.conf"),
    ];
    for p in &candidates {
        if p.exists() {
            eprintln!("loading config from {}", p.display());
            return Config::load(p);
        }
    }
    eprintln!("no flyo-proxy.conf found; using defaults (plain HTTP, no rate limit)");
    Ok(Config::default())
}
