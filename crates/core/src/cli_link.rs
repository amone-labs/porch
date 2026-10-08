//! The `porch` command on PATH: a symlink in `/usr/local/bin` to the CLI the
//! app bundle carries, so it follows app updates without being copied again.
//!
//! A `porch` there that is not a link to some `Porch.app` is someone else's
//! and is never replaced or removed.

use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const LINK: &str = "/usr/local/bin/porch";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Nothing at the link path.
    Missing,
    /// Our link, pointing at this app's CLI.
    Linked,
    /// Our link, pointing at another copy of the app (moved, or a dev build).
    Stale,
    /// A `porch` we did not put there.
    Other,
}

/// A link target that some copy of the app put there; `pnpm tauri dev` runs
/// the app from cargo's debug dir, so a dev build links there. A link to
/// `target/release/porch` is more likely made by hand and stays someone else's.
fn is_app_cli(target: &Path) -> bool {
    target.ends_with("Contents/MacOS/porch") || target.ends_with("target/debug/porch")
}

pub fn status_at(link: &Path, cli: &Path) -> Status {
    let Ok(meta) = fs::symlink_metadata(link) else {
        return Status::Missing;
    };
    if !meta.file_type().is_symlink() {
        return Status::Other;
    }
    match fs::read_link(link) {
        Ok(t) if t == cli => Status::Linked,
        Ok(t) if is_app_cli(&t) => Status::Stale,
        _ => Status::Other,
    }
}

/// Point the link at `cli`. Fails with `PermissionDenied` when the folder
/// needs an administrator, so the caller can ask for one.
pub fn link_at(link: &Path, cli: &Path) -> io::Result<()> {
    match status_at(link, cli) {
        Status::Linked => return Ok(()),
        Status::Other => return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists and is not porch's", link.display()))),
        Status::Stale => fs::remove_file(link)?,
        Status::Missing => {}
    }
    if let Some(d) = link.parent() {
        fs::create_dir_all(d)?;
    }
    std::os::unix::fs::symlink(cli, link)
}

/// Remove the link if it is ours; leave anything else alone.
pub fn unlink_at(link: &Path, cli: &Path) -> io::Result<()> {
    match status_at(link, cli) {
        Status::Linked | Status::Stale => fs::remove_file(link),
        Status::Missing | Status::Other => Ok(()),
    }
}

pub fn link_path() -> PathBuf {
    PathBuf::from(LINK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_unlinks() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("bin/porch");
        let cli = dir.path().join("Porch.app/Contents/MacOS/porch");
        assert_eq!(status_at(&link, &cli), Status::Missing);
        link_at(&link, &cli).unwrap();
        assert_eq!(status_at(&link, &cli), Status::Linked);
        link_at(&link, &cli).unwrap();
        unlink_at(&link, &cli).unwrap();
        assert_eq!(status_at(&link, &cli), Status::Missing);
    }

    #[test]
    fn replaces_a_link_to_another_copy() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("porch");
        let old = dir.path().join("Old/Porch.app/Contents/MacOS/porch");
        let cli = dir.path().join("Porch.app/Contents/MacOS/porch");
        std::os::unix::fs::symlink(&old, &link).unwrap();
        assert_eq!(status_at(&link, &cli), Status::Stale);
        link_at(&link, &cli).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), cli);

        let dev = dir.path().join("porch-x/target/debug/porch");
        unlink_at(&link, &cli).unwrap();
        std::os::unix::fs::symlink(&dev, &link).unwrap();
        assert_eq!(status_at(&link, &cli), Status::Stale);

        let by_hand = dir.path().join("porch-x/target/release/porch");
        unlink_at(&link, &cli).unwrap();
        std::os::unix::fs::symlink(&by_hand, &link).unwrap();
        assert_eq!(status_at(&link, &cli), Status::Other);
    }

    #[test]
    fn never_touches_someone_elses_porch() {
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("Porch.app/Contents/MacOS/porch");
        let file = dir.path().join("porch");
        fs::write(&file, "other").unwrap();
        let link = dir.path().join("porch-link");
        std::os::unix::fs::symlink("/opt/homebrew/bin/porch", &link).unwrap();
        for p in [&file, &link] {
            assert_eq!(status_at(p, &cli), Status::Other);
            assert_eq!(link_at(p, &cli).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
            unlink_at(p, &cli).unwrap();
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), "other");
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("/opt/homebrew/bin/porch"));
    }
}
