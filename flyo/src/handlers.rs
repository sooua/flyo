//! HTTP handlers for the file API.
//!
//! Permission gates here mirror webd's tag semantics:
//!   /api/list    requires `l`
//!   /api/file    requires `r` (Range-aware download)
//!   /api/upload  requires `u`
//!   /api/mkdir   requires `u`
//!   /api/rename  requires `m`
//!   /api/delete  requires `m`
//!
//! All `path` query parameters are routed through `fs_ops::resolve`, which
//! is the single chokepoint preventing path traversal across the API.

use std::path::PathBuf;

use axum::Json;
use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::stream::TryStreamExt;
use mime_guess::MimeGuess;
use serde::{Deserialize, Serialize};
use tokio::fs as tfs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio_util::io::ReaderStream;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::fs_ops::{self, FsError};

/// Convert FsError into an HTTP response so handlers can use `?`.
impl IntoResponse for FsError {
    fn into_response(self) -> Response {
        let code = match self {
            FsError::Traversal => StatusCode::BAD_REQUEST,
            FsError::NotFound => StatusCode::NOT_FOUND,
            FsError::NotADirectory | FsError::NotAFile => StatusCode::BAD_REQUEST,
            FsError::AlreadyExists => StatusCode::CONFLICT,
            FsError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let msg = self.to_string();
        (code, msg).into_response()
    }
}

fn forbid(reason: &str) -> Response {
    (StatusCode::FORBIDDEN, reason.to_string()).into_response()
}

// ---------- list ----------

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    #[serde(default = "default_path")]
    pub path: String,
}

fn default_path() -> String {
    "/".to_string()
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub path: String,
    pub entries: Vec<fs_ops::Entry>,
}

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PathQuery>,
) -> Response {
    if !user.perms.list {
        return forbid("missing 'l' permission");
    }
    match fs_ops::list_dir(&state.cfg, &q.path, user.perms.show_hidden) {
        Ok(entries) => Json(ListResponse {
            path: q.path,
            entries,
        })
        .into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------- file (Range-aware download) ----------

pub async fn file(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PathQuery>,
    headers: HeaderMap,
) -> Response {
    if !user.perms.access {
        return forbid("missing 'r' permission");
    }

    let abs: PathBuf = match fs_ops::resolve(&state.cfg, &q.path) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let meta = match tfs::symlink_metadata(&abs).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return FsError::NotFound.into_response();
        }
        Err(e) => return FsError::Io(e).into_response(),
    };
    if !meta.is_file() {
        return FsError::NotAFile.into_response();
    }
    let total = meta.len();
    let mime = MimeGuess::from_path(&abs).first_or_octet_stream();

    let mut file = match tfs::File::open(&abs).await {
        Ok(f) => f,
        Err(e) => return FsError::Io(e).into_response(),
    };

    let (start, end, status) = match parse_range(headers.get(header::RANGE), total) {
        Some(Ok((s, e))) => (s, e, StatusCode::PARTIAL_CONTENT),
        Some(Err(())) => {
            // Range header was present but malformed/unsatisfiable.
            let mut resp = (StatusCode::RANGE_NOT_SATISFIABLE, "").into_response();
            resp.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{total}")).unwrap(),
            );
            return resp;
        }
        None => (0, total.saturating_sub(1), StatusCode::OK),
    };

    if start > 0 {
        if let Err(e) = file.seek(SeekFrom::Start(start)).await {
            return FsError::Io(e).into_response();
        }
    }
    let length = end.saturating_sub(start).saturating_add(1);
    let limited = file.take(length);
    let stream = ReaderStream::new(limited).map_err(std::io::Error::other);
    let body = Body::from_stream(stream);

    let mut resp = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CONTENT_LENGTH, length)
        .header(header::ACCEPT_RANGES, "bytes");
    if status == StatusCode::PARTIAL_CONTENT {
        resp = resp.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    if user.perms.force_download {
        // 'D' tag: hint browsers to download instead of inlining.
        let fname = abs
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into());
        resp = resp.header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", fname.replace('"', "")),
        );
    }
    resp.body(body).unwrap()
}

/// Parse a single `Range: bytes=start-end` header.
/// Returns `Some(Ok((start, end)))` if satisfiable, `Some(Err(()))` if the
/// header was present but malformed/out-of-range, `None` if no header.
fn parse_range(h: Option<&HeaderValue>, total: u64) -> Option<Result<(u64, u64), ()>> {
    let h = h?;
    let s = h.to_str().ok()?;
    let rest = s.strip_prefix("bytes=")?;
    // Multi-range is rarely used and complicates streaming; serve the first range only.
    let first = rest.split(',').next()?.trim();
    let (a, b) = first.split_once('-')?;

    let (start, end) = if a.is_empty() {
        // Suffix range: "-N" → last N bytes
        let n: u64 = b.parse().ok()?;
        if n == 0 || n > total {
            return Some(Err(()));
        }
        (total - n, total - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end: u64 = if b.is_empty() {
            total.saturating_sub(1)
        } else {
            b.parse().ok()?
        };
        if start > end || end >= total {
            return Some(Err(()));
        }
        (start, end)
    };
    Some(Ok((start, end)))
}

// ---------- upload ----------

pub async fn upload(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PathQuery>,
    req: Request,
) -> Response {
    if !user.perms.upload {
        return forbid("missing 'u' permission");
    }
    let abs = match fs_ops::resolve(&state.cfg, &q.path) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    // Ensure parent dir exists; webd implicitly creates intermediate paths.
    if let Some(parent) = abs.parent() {
        if !parent.exists() {
            if let Err(e) = tfs::create_dir_all(parent).await {
                return FsError::Io(e).into_response();
            }
        }
    }

    // Stream body into a temp sibling, then atomic rename. Avoids leaving
    // half-uploaded files visible in the listing if the connection drops.
    let tmp = abs.with_extension({
        let mut ext = abs
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        if !ext.is_empty() {
            ext.push('.');
        }
        format!("{ext}flyo-tmp-{}", rand_suffix())
    });

    let mut file = match tfs::File::create(&tmp).await {
        Ok(f) => f,
        Err(e) => return FsError::Io(e).into_response(),
    };

    let mut stream = req.into_body().into_data_stream();
    let mut bytes_written: u64 = 0;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(_) => {
                let _ = tfs::remove_file(&tmp).await;
                return (StatusCode::BAD_REQUEST, "stream error").into_response();
            }
        };
        if let Err(e) = file.write_all(&chunk).await {
            let _ = tfs::remove_file(&tmp).await;
            return FsError::Io(e).into_response();
        }
        bytes_written += chunk.len() as u64;
    }
    if let Err(e) = file.flush().await {
        let _ = tfs::remove_file(&tmp).await;
        return FsError::Io(e).into_response();
    }
    drop(file);

    if let Err(e) = tfs::rename(&tmp, &abs).await {
        let _ = tfs::remove_file(&tmp).await;
        return FsError::Io(e).into_response();
    }

    let body = serde_json::json!({"path": q.path, "bytes": bytes_written});
    (StatusCode::OK, Json(body)).into_response()
}

fn rand_suffix() -> String {
    use rand::Rng;
    const A: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| A[rng.random_range(0..A.len())] as char)
        .collect()
}

// ---------- mkdir ----------

pub async fn mkdir(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PathQuery>,
) -> Response {
    if !user.perms.upload {
        return forbid("missing 'u' permission");
    }
    match fs_ops::mkdir(&state.cfg, &q.path) {
        Ok(()) => (StatusCode::OK, "ok").into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------- rename ----------

#[derive(Debug, Deserialize)]
pub struct RenameQuery {
    pub from: String,
    pub to: String,
}

pub async fn rename(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<RenameQuery>,
) -> Response {
    if !user.perms.modify {
        return forbid("missing 'm' permission");
    }
    match fs_ops::rename(&state.cfg, &q.from, &q.to) {
        Ok(()) => (StatusCode::OK, "ok").into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------- delete ----------

pub async fn delete(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PathQuery>,
) -> Response {
    if !user.perms.modify {
        return forbid("missing 'm' permission");
    }
    match fs_ops::delete_to_trash(&state.cfg, &q.path) {
        Ok(()) => (StatusCode::OK, "ok").into_response(),
        Err(e) => e.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_range() {
        let h = HeaderValue::from_static("bytes=0-99");
        assert_eq!(parse_range(Some(&h), 200), Some(Ok((0, 99))));
    }

    #[test]
    fn parse_open_range() {
        let h = HeaderValue::from_static("bytes=50-");
        assert_eq!(parse_range(Some(&h), 200), Some(Ok((50, 199))));
    }

    #[test]
    fn parse_suffix_range() {
        let h = HeaderValue::from_static("bytes=-30");
        assert_eq!(parse_range(Some(&h), 200), Some(Ok((170, 199))));
    }

    #[test]
    fn parse_first_of_multi_range() {
        let h = HeaderValue::from_static("bytes=0-9,20-29");
        assert_eq!(parse_range(Some(&h), 100), Some(Ok((0, 9))));
    }

    #[test]
    fn rejects_unsatisfiable_range() {
        let h = HeaderValue::from_static("bytes=500-600");
        assert_eq!(parse_range(Some(&h), 100), Some(Err(())));
    }

    #[test]
    fn rejects_backwards_range() {
        let h = HeaderValue::from_static("bytes=99-0");
        assert_eq!(parse_range(Some(&h), 200), Some(Err(())));
    }

    #[test]
    fn no_header_means_no_range() {
        assert_eq!(parse_range(None, 200), None);
    }
}
