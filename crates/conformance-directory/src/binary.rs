use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::error::{ConformanceError, Result};
use crate::release_asset::{Remote, install_from_release};

pub const ANCHOR_BINARY: &str = "identity-directory";

pub async fn locate_anchor() -> Result<PathBuf> {
    let path = std::env::var_os("PATH");
    let cache = cache_dir_from(|name| std::env::var_os(name));
    locate(
        ANCHOR_BINARY,
        path.as_deref(),
        cache.as_deref(),
        Some(&Remote::release()),
    )
    .await
}

pub(crate) async fn locate(
    binary: &'static str,
    path: Option<&OsStr>,
    cache: Option<&Path>,
    remote: Option<&Remote>,
) -> Result<PathBuf> {
    if let Some(found) = path.and_then(|path| find_in_path(binary, path)) {
        return Ok(found);
    }
    let Some(cache) = cache else {
        return Err(unavailable(
            binary,
            "no cache directory: XDG_CACHE_HOME and HOME are not set to an absolute path",
        ));
    };
    let cached = cache.join(binary);
    if is_executable_file(&cached) {
        return Ok(cached);
    }
    let Some(remote) = remote else {
        return Err(unavailable(binary, "release download is not attempted"));
    };
    install_from_release(binary, cache, remote)
        .await
        .map_err(|detail| unavailable(binary, &detail))?;
    Ok(cached)
}

pub(crate) fn unavailable(binary: &'static str, detail: &str) -> ConformanceError {
    ConformanceError::AnchorBinaryUnavailable {
        binary,
        detail: detail.to_string(),
    }
}

pub(crate) fn cache_dir_from(lookup: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| {
        lookup(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let root = match absolute("XDG_CACHE_HOME") {
        Some(xdg) => xdg,
        None => {
            let home = absolute("HOME")?;
            if cfg!(target_os = "macos") {
                home.join("Library").join("Caches")
            } else {
                home.join(".cache")
            }
        }
    };
    Some(root.join("br-e2e-harness").join(format!(
        "conformance-directory-{}",
        env!("CARGO_PKG_VERSION")
    )))
}

pub(crate) fn absolute_entries(path: &OsStr) -> Vec<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .collect()
}

pub(crate) fn find_in_path(binary: &str, path: &OsStr) -> Option<PathBuf> {
    absolute_entries(path)
        .into_iter()
        .map(|dir| dir.join(binary))
        .find(|candidate| is_executable_file(candidate))
}

#[cfg(unix)]
pub(crate) fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
pub(crate) fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

pub(crate) fn path_with_directory_first(directory: &Path, path: Option<&OsStr>) -> Result<String> {
    let mut entries = vec![directory.to_path_buf()];
    if let Some(path) = path {
        entries.extend(std::env::split_paths(path));
    }
    std::env::join_paths(entries)
        .ok()
        .and_then(|joined| joined.into_string().ok())
        .ok_or_else(|| {
            ConformanceError::Run("PATH cannot be extended with the anchor directory".to_string())
        })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::test_scratch::scratch;

    fn write_binary(dir: &Path, name: &str, mode: u32) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    #[test]
    fn an_executable_on_path_is_found() {
        let dir = scratch("path-hit");
        let binary = write_binary(&dir, ANCHOR_BINARY, 0o755);
        let path = std::env::join_paths([&dir]).unwrap();
        assert_eq!(find_in_path(ANCHOR_BINARY, &path), Some(binary));
    }

    #[test]
    fn a_non_executable_file_is_ignored() {
        let dir = scratch("path-miss");
        write_binary(&dir, ANCHOR_BINARY, 0o644);
        let path = std::env::join_paths([&dir]).unwrap();
        assert_eq!(find_in_path(ANCHOR_BINARY, &path), None);
    }

    #[test]
    fn only_absolute_path_entries_are_searched() {
        let path = OsStr::new("relative:.::/usr/bin:../up:/bin");
        assert_eq!(
            absolute_entries(path),
            [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
        );
        assert!(absolute_entries(OsStr::new("")).is_empty());
    }

    #[tokio::test]
    async fn path_wins_over_the_cache() {
        let on_path = scratch("wins-path");
        let cache = scratch("wins-cache");
        let expected = write_binary(&on_path, ANCHOR_BINARY, 0o755);
        write_binary(&cache, ANCHOR_BINARY, 0o755);
        let path = std::env::join_paths([&on_path]).unwrap();
        let found = locate(ANCHOR_BINARY, Some(&path), Some(&cache), None)
            .await
            .unwrap();
        assert_eq!(found, expected);
    }

    #[tokio::test]
    async fn the_cache_is_used_when_path_misses() {
        let cache = scratch("cache-hit");
        let expected = write_binary(&cache, ANCHOR_BINARY, 0o755);
        let found = locate(ANCHOR_BINARY, None, Some(&cache), None)
            .await
            .unwrap();
        assert_eq!(found, expected);
    }

    #[tokio::test]
    async fn a_missing_binary_is_a_typed_error_naming_it() {
        let cache = scratch("missing");
        match locate(ANCHOR_BINARY, None, Some(&cache), None).await {
            Err(error @ ConformanceError::AnchorBinaryUnavailable { .. }) => {
                let message = error.to_string();
                assert!(message.contains(ANCHOR_BINARY), "{message}");
                assert!(message.contains("PATH"), "{message}");
            }
            other => panic!("expected a typed missing-binary error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_cache_directory_is_a_typed_error() {
        assert!(matches!(
            locate(ANCHOR_BINARY, None, None, None).await,
            Err(ConformanceError::AnchorBinaryUnavailable { .. })
        ));
    }

    #[test]
    fn the_cache_directory_follows_xdg_then_home() {
        let version = env!("CARGO_PKG_VERSION");
        let tail = PathBuf::from("br-e2e-harness").join(format!("conformance-directory-{version}"));
        let xdg = cache_dir_from(|name| (name == "XDG_CACHE_HOME").then(|| "/xdg".into()));
        assert_eq!(xdg, Some(PathBuf::from("/xdg").join(&tail)));
        let home = cache_dir_from(|name| (name == "HOME").then(|| "/home/u".into())).unwrap();
        assert!(home.starts_with("/home/u") && home.ends_with(&tail));
        assert_eq!(cache_dir_from(|_| None), None);
    }

    #[test]
    fn a_relative_xdg_cache_home_is_ignored_in_favour_of_home() {
        let version = env!("CARGO_PKG_VERSION");
        let tail = PathBuf::from("br-e2e-harness").join(format!("conformance-directory-{version}"));
        let found = cache_dir_from(|name| match name {
            "XDG_CACHE_HOME" => Some("relative/cache".into()),
            "HOME" => Some("/home/u".into()),
            _ => None,
        })
        .unwrap();
        assert!(found.is_absolute());
        assert!(found.starts_with("/home/u") && found.ends_with(&tail));
        assert!(!found.starts_with("relative"));
    }

    #[test]
    fn a_relative_home_gives_no_cache_directory() {
        assert_eq!(
            cache_dir_from(|name| (name == "HOME").then(|| "relative/home".into())),
            None
        );
        assert_eq!(
            cache_dir_from(|name| match name {
                "XDG_CACHE_HOME" => Some("relative/cache".into()),
                "HOME" => Some("relative/home".into()),
                _ => None,
            }),
            None
        );
    }

    #[test]
    fn the_anchor_directory_leads_the_child_path() {
        let joined =
            path_with_directory_first(Path::new("/cache"), Some(OsStr::new("/usr/bin:/bin")))
                .unwrap();
        assert_eq!(joined, "/cache:/usr/bin:/bin");
    }
}
