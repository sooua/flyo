//! Filesystem operations exposed by the HTTP API.
//!
//! Every public function takes a `cfg: &Config` and a virtual `path` string
//! (the URL `?path=...` parameter, always interpreted relative to `cfg.root`).
//! Path resolution rejects anything that would escape the share root via
//! `..`, absolute paths, or symlinks. This is the single chokepoint that
//! prevents path-traversal across the API surface.

use std::fs::{self, Metadata};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("path is outside the share root")]
    Traversal,
    #[error("not found")]
    NotFound,
    #[error("not a directory")]
    NotADirectory,
    #[error("not a regular file")]
    NotAFile,
    #[error("already exists")]
    AlreadyExists,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Serialize, Clone)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Modification time, seconds since UNIX epoch.
    pub mtime: i64,
}

/// Resolve a user-supplied virtual path against the configured share root.
///
/// The returned `PathBuf` is guaranteed to live inside `cfg.root`. Returns
/// `Traversal` if the request tries to escape via `..`, absolute paths, or
/// drive prefixes (Windows).
pub fn resolve(cfg: &Config, virtual_path: &str) -> Result<PathBuf, FsError> {
    let raw = virtual_path.trim_start_matches('/');
    let virt = Path::new(raw);

    let mut safe = PathBuf::new();
    for comp in virt.components() {
        match comp {
            Component::Normal(part) => safe.push(part),
            // Treat the root as the current location; ignore.
            Component::CurDir | Component::RootDir => continue,
            // Reject anything that would walk up or hop drives.
            Component::ParentDir | Component::Prefix(_) => return Err(FsError::Traversal),
        }
    }
    Ok(cfg.root.join(safe))
}

/// List a directory's contents.
pub fn list_dir(cfg: &Config, virtual_path: &str, show_hidden: bool) -> Result<Vec<Entry>, FsError> {
    let abs = resolve(cfg, virtual_path)?;
    let meta = match fs::symlink_metadata(&abs) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(FsError::NotFound),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_dir() {
        return Err(FsError::NotADirectory);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&abs)? {
        let entry = entry?;
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            // Skip names that aren't valid UTF-8; we serve over UTF-8 JSON.
            Err(_) => continue,
        };
        // Skip the recycle-bin folder webd uses; never show it in listings.
        if name == ".Trash" {
            continue;
        }
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue, // file might disappear between read_dir and metadata
        };
        out.push(entry_from(name, &md));
    }
    // Stable ordering: directories first, then by name.
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

fn entry_from(name: String, md: &Metadata) -> Entry {
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Entry {
        is_dir: md.is_dir(),
        size: if md.is_dir() { 0 } else { md.len() },
        mtime,
        name,
    }
}

pub fn mkdir(cfg: &Config, virtual_path: &str) -> Result<(), FsError> {
    let abs = resolve(cfg, virtual_path)?;
    if abs.exists() {
        return Err(FsError::AlreadyExists);
    }
    fs::create_dir_all(&abs)?;
    Ok(())
}

pub fn rename(cfg: &Config, from: &str, to: &str) -> Result<(), FsError> {
    let a = resolve(cfg, from)?;
    let b = resolve(cfg, to)?;
    if !a.exists() {
        return Err(FsError::NotFound);
    }
    if let Some(parent) = b.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::rename(&a, &b)?;
    Ok(())
}

/// Move an entry into the share's `.Trash/` directory, prefixing the name
/// with a timestamp so concurrent deletes can't clash.
pub fn delete_to_trash(cfg: &Config, virtual_path: &str) -> Result<(), FsError> {
    let abs = resolve(cfg, virtual_path)?;
    if !abs.exists() {
        return Err(FsError::NotFound);
    }
    let trash = cfg.root.join(".Trash");
    fs::create_dir_all(&trash)?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let basename = abs
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".into());
    let target = trash.join(format!(".{stamp:x}.{basename}"));
    fs::rename(&abs, &target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;

    fn tmpdir(tag: &str) -> PathBuf {
        let mut p = env::temp_dir();
        p.push(format!(
            "flyo-fs-test-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn cfg_in(root: &Path) -> Config {
        Config {
            root: root.to_path_buf(),
            ..Config::default()
        }
    }

    #[test]
    fn resolve_normalises_leading_slash() {
        let root = tmpdir("res1");
        let cfg = cfg_in(&root);
        assert_eq!(resolve(&cfg, "/foo").unwrap(), root.join("foo"));
        assert_eq!(resolve(&cfg, "foo").unwrap(), root.join("foo"));
        assert_eq!(resolve(&cfg, "/").unwrap(), root);
    }

    #[test]
    fn resolve_rejects_dotdot() {
        let root = tmpdir("res2");
        let cfg = cfg_in(&root);
        assert!(matches!(resolve(&cfg, "/../etc/passwd"), Err(FsError::Traversal)));
        assert!(matches!(resolve(&cfg, "foo/../../bar"), Err(FsError::Traversal)));
    }

    #[test]
    #[cfg(windows)]
    fn resolve_rejects_drive_prefix() {
        let root = tmpdir("res3");
        let cfg = cfg_in(&root);
        assert!(matches!(resolve(&cfg, "C:\\Windows"), Err(FsError::Traversal)));
    }

    #[test]
    fn list_dir_hides_hidden_and_trash_by_default() {
        let root = tmpdir("list1");
        File::create(root.join("visible.txt")).unwrap();
        File::create(root.join(".secret")).unwrap();
        fs::create_dir_all(root.join(".Trash")).unwrap();
        File::create(root.join(".Trash/.0.foo")).unwrap();

        let cfg = cfg_in(&root);
        let entries = list_dir(&cfg, "/", false).unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["visible.txt"]);
    }

    #[test]
    fn list_dir_shows_hidden_when_allowed_but_never_trash() {
        let root = tmpdir("list2");
        File::create(root.join("v.txt")).unwrap();
        File::create(root.join(".secret")).unwrap();
        fs::create_dir_all(root.join(".Trash")).unwrap();

        let cfg = cfg_in(&root);
        let entries = list_dir(&cfg, "/", true).unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".secret"));
        assert!(names.contains(&"v.txt"));
        assert!(!names.contains(&".Trash"), "trash must never be listed");
    }

    #[test]
    fn list_dir_orders_directories_first_then_alphabetical() {
        let root = tmpdir("list3");
        File::create(root.join("zfile.txt")).unwrap();
        fs::create_dir_all(root.join("adir")).unwrap();
        File::create(root.join("bfile.txt")).unwrap();

        let cfg = cfg_in(&root);
        let entries = list_dir(&cfg, "/", false).unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["adir", "bfile.txt", "zfile.txt"]);
    }

    #[test]
    fn mkdir_creates_nested_paths() {
        let root = tmpdir("mk1");
        let cfg = cfg_in(&root);
        mkdir(&cfg, "/a/b/c").unwrap();
        assert!(root.join("a/b/c").is_dir());
    }

    #[test]
    fn mkdir_rejects_existing_path() {
        let root = tmpdir("mk2");
        let cfg = cfg_in(&root);
        fs::create_dir_all(root.join("exists")).unwrap();
        assert!(matches!(mkdir(&cfg, "/exists"), Err(FsError::AlreadyExists)));
    }

    #[test]
    fn rename_moves_and_creates_parents() {
        let root = tmpdir("rn1");
        let cfg = cfg_in(&root);
        let mut f = File::create(root.join("old.txt")).unwrap();
        writeln!(f, "hello").unwrap();
        rename(&cfg, "/old.txt", "/sub/new.txt").unwrap();
        assert!(!root.join("old.txt").exists());
        assert!(root.join("sub/new.txt").exists());
    }

    #[test]
    fn delete_moves_into_trash_with_unique_name() {
        let root = tmpdir("del1");
        let cfg = cfg_in(&root);
        File::create(root.join("a.txt")).unwrap();
        delete_to_trash(&cfg, "/a.txt").unwrap();
        assert!(!root.join("a.txt").exists());
        let trash = root.join(".Trash");
        let entries: Vec<_> = fs::read_dir(&trash)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1);
        let name = entries[0].file_name().to_string_lossy().to_string();
        assert!(name.ends_with(".a.txt"), "trash entry name = {name}");
    }
}
