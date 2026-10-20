use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;
use reqwest::Url;
use reqwest::redirect::Policy as RedirectPolicy;
use sha2::{Digest, Sha256};

use crate::binary::find_in_path;
use crate::cause::describe;

const RELEASE_BASE: &str = "https://github.com/BotResources/br-e2e-harness/releases/download";
const RELEASE_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CHECKSUM_BYTES: usize = 1024;
const MAX_REDIRECTS: usize = 5;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);
const EXTRACT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
pub(crate) struct Policy {
    pub(crate) https_only: bool,
    pub(crate) system_proxy: bool,
    pub(crate) hosts: &'static [&'static str],
    pub(crate) max_archive_bytes: usize,
    pub(crate) max_checksum_bytes: usize,
}

impl Policy {
    fn permits(&self, url: &Url) -> std::result::Result<(), String> {
        if self.https_only && url.scheme() != "https" {
            return Err(format!("{url} is not an https URL"));
        }
        match url.host_str() {
            Some(host) if self.hosts.contains(&host) => Ok(()),
            Some(host) => Err(format!("{host} is not an allowed download host")),
            None => Err(format!("{url} has no host")),
        }
    }
}

pub(crate) struct Remote {
    pub(crate) base_url: String,
    pub(crate) policy: Policy,
}

impl Remote {
    pub(crate) fn release() -> Self {
        Self {
            base_url: format!(
                "{RELEASE_BASE}/conformance-passport%2Fv{}",
                env!("CARGO_PKG_VERSION")
            ),
            policy: Policy {
                https_only: true,
                system_proxy: true,
                hosts: RELEASE_HOSTS,
                max_archive_bytes: MAX_ARCHIVE_BYTES,
                max_checksum_bytes: MAX_CHECKSUM_BYTES,
            },
        }
    }
}

pub(crate) fn platform_suffix(os: &str, arch: &str) -> std::result::Result<String, String> {
    let os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        other => return Err(format!("the operating system {other} has no release asset")),
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => return Err(format!("the architecture {other} has no release asset")),
    };
    Ok(format!("{os}-{arch}"))
}

pub(crate) fn asset_name(
    binary: &str,
    os: &str,
    arch: &str,
) -> std::result::Result<String, String> {
    Ok(format!("{binary}-{}.tar.gz", platform_suffix(os, arch)?))
}

pub(crate) fn verify_checksum(
    archive: &[u8],
    checksum_file: &[u8],
) -> std::result::Result<(), String> {
    let text = std::str::from_utf8(checksum_file)
        .map_err(|_| "the checksum file is not UTF-8".to_string())?;
    let expected = text
        .split_whitespace()
        .next()
        .filter(|token| token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| "the checksum file does not start with a SHA-256 hex digest".to_string())?
        .to_ascii_lowercase();
    let actual = hex_digest(archive);
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "SHA-256 mismatch: the release publishes {expected}, the download hashes to {actual}"
        ))
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) async fn install_from_release(
    binary: &'static str,
    cache: &Path,
    remote: &Remote,
) -> std::result::Result<(), String> {
    let asset = asset_name(binary, std::env::consts::OS, std::env::consts::ARCH)?;
    let client = client(remote.policy)?;
    let archive_url = format!("{}/{asset}", remote.base_url);
    let checksum_url = format!("{archive_url}.sha256");
    let archive = fetch_capped(
        &client,
        remote.policy,
        &archive_url,
        remote.policy.max_archive_bytes,
    )
    .await
    .map_err(|e| format!("downloading {asset} failed: {e}"))?;
    let checksum = fetch_capped(
        &client,
        remote.policy,
        &checksum_url,
        remote.policy.max_checksum_bytes,
    )
    .await
    .map_err(|e| format!("downloading {asset}.sha256 failed: {e}"))?;
    verify_checksum(&archive, &checksum).map_err(|e| format!("{asset}: {e}"))?;
    install_archive(binary, cache, &archive).await
}

fn client(policy: Policy) -> std::result::Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(RedirectPolicy::custom(move |attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                return attempt.error("too many redirects");
            }
            match policy.permits(attempt.url()) {
                Ok(()) => attempt.follow(),
                Err(reason) => attempt.error(format!("redirect refused: {reason}")),
            }
        }));
    let builder = if policy.system_proxy {
        builder
    } else {
        builder.no_proxy()
    };
    builder.build().map_err(|e| describe(&e))
}

async fn fetch_capped(
    client: &reqwest::Client,
    policy: Policy,
    url: &str,
    cap: usize,
) -> std::result::Result<Vec<u8>, String> {
    let parsed = Url::parse(url).map_err(|e| format!("{url}: {e}"))?;
    policy.permits(&parsed)?;
    let mut response = client.get(parsed).send().await.map_err(|e| describe(&e))?;
    if !response.status().is_success() {
        return Err(format!("{url} answered {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|len| len > cap as u64)
    {
        return Err(format!("{url} announces more than {cap} bytes"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| describe(&e))? {
        if body.len() + chunk.len() > cap {
            return Err(format!("{url} streams more than {cap} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn install_archive(
    binary: &'static str,
    cache: &Path,
    archive: &[u8],
) -> std::result::Result<(), String> {
    let staging = cache.join(format!(".staging-{}", uuid::Uuid::now_v7().simple()));
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|e| format!("cannot create {}: {e}", staging.display()))?;
    let outcome = extract_into_cache(binary, cache, &staging, archive).await;
    tokio::fs::remove_dir_all(&staging).await.ok();
    outcome
}

async fn extract_into_cache(
    binary: &'static str,
    cache: &Path,
    staging: &Path,
    archive: &[u8],
) -> std::result::Result<(), String> {
    let archive_path = staging.join("archive.tar.gz");
    tokio::fs::write(&archive_path, archive)
        .await
        .map_err(|e| format!("cannot write {}: {e}", archive_path.display()))?;
    let tar = tar_program(std::env::var_os("PATH").as_deref())?;
    let output = run_once(
        &tar.to_string_lossy(),
        &[
            "-xzf",
            &archive_path.to_string_lossy(),
            "-C",
            &staging.to_string_lossy(),
            binary,
        ],
        &[],
        EXTRACT_TIMEOUT,
    )
    .await?;
    if !output.status.success() {
        return Err(format!(
            "tar could not extract {binary} ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let extracted = staging.join(binary);
    let meta = tokio::fs::symlink_metadata(&extracted)
        .await
        .map_err(|e| format!("the archive does not carry {binary}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("the archive entry {binary} is not a regular file"));
    }
    make_executable(&extracted).await?;
    tokio::fs::rename(&extracted, cache.join(binary))
        .await
        .map_err(|e| format!("cannot move {binary} into {}: {e}", cache.display()))
}

pub(crate) fn tar_program(path: Option<&std::ffi::OsStr>) -> std::result::Result<PathBuf, String> {
    path.and_then(|path| find_in_path("tar", path)).ok_or_else(|| {
        "tar is not an executable in an absolute PATH entry: install tar or put the binary in the cache directory".to_string()
    })
}

#[cfg(unix)]
async fn make_executable(path: &Path) -> std::result::Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .await
        .map_err(|e| format!("cannot mark {} executable: {e}", path.display()))
}

#[cfg(not(unix))]
async fn make_executable(_path: &Path) -> std::result::Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests;
