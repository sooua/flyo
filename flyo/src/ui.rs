//! Embed the built frontend assets (web/dist/) into the binary and serve them.
//!
//! `rust-embed` walks the folder at compile time and bakes file bytes into the
//! binary (gzip-compressed because of the "compression" feature). At runtime
//! we decompress on demand and serve with the appropriate mime + caching.
//!
//! Routing strategy: any GET that does NOT match an /api/* or /__* route is
//! handled here. If the path matches an embedded file → serve it. Otherwise
//! fall back to index.html so the SPA's hash router can take over.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn serve(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/');
    // Empty path → index. Treat any unmatched route as SPA fallback.
    let candidate = if path.is_empty() { "index.html" } else { path };

    match Assets::get(candidate) {
        Some(file) => render(candidate, file),
        None => match Assets::get("index.html") {
            Some(file) => render("index.html", file),
            None => (
                StatusCode::SERVICE_UNAVAILABLE,
                "UI assets not embedded. Build the frontend (`pnpm --dir web build`) and recompile.",
            )
                .into_response(),
        },
    }
}

fn render(name: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    // index.html must never be cached so deploys flip atomically.
    // Hashed asset filenames (Vite default) can cache forever.
    let cache = if name == "index.html" {
        "no-cache, must-revalidate"
    } else if name.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=300"
    };

    let body = Body::from(file.data);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
        .body(body)
        .unwrap()
}
