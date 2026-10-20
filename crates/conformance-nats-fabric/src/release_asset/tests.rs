use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;
use crate::binary::{ANCHOR_BINARY, is_executable_file, locate};
use crate::error::ConformanceError;
use crate::test_scratch::{assert_child_ran_one_passing_test, scratch};

const LOCAL: Policy = Policy {
    https_only: false,
    system_proxy: false,
    hosts: &["127.0.0.1"],
    max_archive_bytes: 4096,
    max_checksum_bytes: 256,
};

#[derive(Clone)]
enum Reply {
    Body(Vec<u8>),
    Redirect(String),
    Announce(usize),
    Stream(Vec<u8>),
}

async fn serve(routes: HashMap<String, Reply>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let routes = routes.clone();
            tokio::spawn(async move {
                let mut request = vec![0u8; 8192];
                let read = socket.read(&mut request).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&request[..read]).into_owned();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let response = match routes.get(&path) {
                    Some(Reply::Body(body)) => {
                        let mut out = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes();
                        out.extend_from_slice(body);
                        out
                    }
                    Some(Reply::Redirect(to)) => format!(
                        "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .into_bytes(),
                    Some(Reply::Announce(len)) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
                    )
                    .into_bytes(),
                    Some(Reply::Stream(body)) => {
                        let mut out = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
                        out.extend_from_slice(body);
                        out
                    }
                    None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_vec(),
                };
                socket.write_all(&response).await.ok();
                socket.shutdown().await.ok();
            });
        }
    });
    port
}

fn asset() -> String {
    asset_name(ANCHOR_BINARY, std::env::consts::OS, std::env::consts::ARCH).unwrap()
}

fn archive_of(dir: &Path, prepare: impl FnOnce(&Path)) -> Vec<u8> {
    let source = dir.join("source");
    std::fs::create_dir_all(&source).unwrap();
    prepare(&source);
    let archive = dir.join("fixture.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&source)
        .arg(ANCHOR_BINARY)
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::read(archive).unwrap()
}

fn script(source: &Path) {
    std::fs::write(source.join(ANCHOR_BINARY), b"#!/bin/sh\necho anchor\n").unwrap();
}

fn checksum(archive: &[u8]) -> Vec<u8> {
    format!("{}  {}\n", hex_digest(archive), asset()).into_bytes()
}

fn remote(port: u16) -> Remote {
    Remote {
        base_url: format!("http://127.0.0.1:{port}"),
        policy: LOCAL,
    }
}

async fn download_into(
    cache: &Path,
    routes: HashMap<String, Reply>,
) -> Result<PathBuf, ConformanceError> {
    let port = serve(routes).await;
    locate(ANCHOR_BINARY, None, Some(cache), Some(&remote(port))).await
}

fn routes_for(archive: Vec<u8>, checksum: Vec<u8>) -> HashMap<String, Reply> {
    HashMap::from([
        (format!("/{}", asset()), Reply::Body(archive)),
        (format!("/{}.sha256", asset()), Reply::Body(checksum)),
    ])
}

fn detail(result: Result<PathBuf, ConformanceError>) -> String {
    match result {
        Err(ConformanceError::AnchorBinaryUnavailable { detail, .. }) => detail,
        other => panic!("expected a typed download failure: {other:?}"),
    }
}

#[test]
fn every_supported_platform_has_one_asset_name() {
    let names: Vec<_> = [
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "x86_64"),
        ("macos", "aarch64"),
    ]
    .iter()
    .map(|(os, arch)| asset_name(ANCHOR_BINARY, os, arch).unwrap())
    .collect();
    assert_eq!(
        names,
        [
            "nats-fabric-linux-amd64.tar.gz",
            "nats-fabric-linux-arm64.tar.gz",
            "nats-fabric-darwin-amd64.tar.gz",
            "nats-fabric-darwin-arm64.tar.gz",
        ]
    );
    assert!(asset_name(ANCHOR_BINARY, "windows", "x86_64").is_err());
    assert!(asset_name(ANCHOR_BINARY, "linux", "riscv64").is_err());
}

#[test]
fn the_release_url_names_this_crate_release_tag() {
    let remote = Remote::release();
    assert_eq!(
        remote.base_url,
        format!(
            "https://github.com/BotResources/br-e2e-harness/releases/download/conformance-nats-fabric%2Fv{}",
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[test]
fn a_matching_digest_verifies_and_any_other_fails_closed() {
    let archive = b"archive bytes";
    let digest = hex_digest(archive);
    assert!(verify_checksum(archive, format!("{digest}  file\n").as_bytes()).is_ok());
    assert!(verify_checksum(archive, digest.to_uppercase().as_bytes()).is_ok());
    assert!(
        verify_checksum(
            archive,
            format!("{}  file", hex_digest(b"other")).as_bytes()
        )
        .is_err()
    );
    assert!(verify_checksum(archive, b"not-a-digest file").is_err());
    assert!(verify_checksum(archive, b"").is_err());
    assert!(verify_checksum(archive, &[0xff, 0xfe]).is_err());
}

#[test]
fn the_release_policy_requires_https_and_a_github_host() {
    let policy = Remote::release().policy;
    let allowed = |url: &str| policy.permits(&Url::parse(url).unwrap());
    assert!(allowed("https://github.com/a").is_ok());
    assert!(allowed("https://objects.githubusercontent.com/a").is_ok());
    assert!(allowed("https://release-assets.githubusercontent.com/a").is_ok());
    assert!(allowed("http://github.com/a").is_err());
    assert!(allowed("https://github.com.evil.example/a").is_err());
    assert!(allowed("https://evil.example/a").is_err());
}

#[tokio::test]
async fn a_verified_archive_installs_an_executable_into_the_cache() {
    let dir = scratch("install-ok");
    let archive = archive_of(&dir, script);
    let cache = dir.join("cache");
    let installed = download_into(&cache, routes_for(archive.clone(), checksum(&archive)))
        .await
        .unwrap();
    assert_eq!(installed, cache.join(ANCHOR_BINARY));
    assert!(is_executable_file(&installed));
    let leftovers: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers,
        [ANCHOR_BINARY],
        "no staging directory is left behind"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_redirect_to_an_allowed_host_is_followed() {
    let dir = scratch("install-follow");
    let archive = archive_of(&dir, script);
    let cache = dir.join("cache");
    let target = serve(routes_for(archive.clone(), checksum(&archive))).await;
    let routes = HashMap::from([
        (
            format!("/{}", asset()),
            Reply::Redirect(format!("http://127.0.0.1:{target}/{}", asset())),
        ),
        (
            format!("/{}.sha256", asset()),
            Reply::Body(checksum(&archive)),
        ),
    ]);
    let installed = download_into(&cache, routes).await.unwrap();
    assert!(is_executable_file(&installed));
    std::fs::remove_dir_all(dir).ok();
}

const PROXIED: &str = "CONFORMANCE_NATS_FABRIC_PROXIED_DOWNLOAD";
const PROXIED_TEST: &str = "release_asset::tests::a_download_ignores_the_system_proxy";

#[test]
fn a_download_ignores_the_system_proxy() {
    if std::env::var_os(PROXIED).is_none() {
        let unreachable_proxy = "http://127.0.0.1:9";
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", PROXIED_TEST, "--nocapture"])
            .env(PROXIED, "1")
            .env("HTTP_PROXY", unreachable_proxy)
            .env("http_proxy", unreachable_proxy)
            .env("ALL_PROXY", unreachable_proxy)
            .env("all_proxy", unreachable_proxy)
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .output()
            .unwrap();
        assert_child_ran_one_passing_test(&output);
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let dir = scratch("install-proxy");
            let archive = archive_of(&dir, script);
            let cache = dir.join("cache");
            let installed = download_into(&cache, routes_for(archive.clone(), checksum(&archive)))
                .await
                .unwrap();
            assert!(is_executable_file(&installed));
            std::fs::remove_dir_all(dir).ok();
        });
}

#[tokio::test]
async fn a_digest_mismatch_installs_nothing() {
    let dir = scratch("install-sha");
    let archive = archive_of(&dir, script);
    let cache = dir.join("cache");
    let wrong = checksum(b"another archive");
    let message = detail(download_into(&cache, routes_for(archive, wrong)).await);
    assert!(message.contains("SHA-256 mismatch"), "{message}");
    assert!(!cache.join(ANCHOR_BINARY).exists());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_redirect_to_a_foreign_host_is_refused() {
    let dir = scratch("install-redirect");
    let cache = dir.join("cache");
    let port = serve(HashMap::new()).await;
    let routes = HashMap::from([(
        format!("/{}", asset()),
        Reply::Redirect(format!("http://localhost:{port}/elsewhere")),
    )]);
    let message = detail(download_into(&cache, routes).await);
    assert!(
        message.contains("not an allowed download host"),
        "{message}"
    );
    assert!(!cache.join(ANCHOR_BINARY).exists());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn an_announced_size_over_the_cap_is_refused() {
    let dir = scratch("install-announce");
    let cache = dir.join("cache");
    let routes = HashMap::from([(
        format!("/{}", asset()),
        Reply::Announce(LOCAL.max_archive_bytes + 1),
    )]);
    let message = detail(download_into(&cache, routes).await);
    assert!(message.contains("announces more than"), "{message}");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_streamed_size_over_the_cap_is_refused() {
    let dir = scratch("install-stream");
    let cache = dir.join("cache");
    let routes = HashMap::from([(
        format!("/{}", asset()),
        Reply::Stream(vec![7u8; LOCAL.max_archive_bytes + 1]),
    )]);
    let message = detail(download_into(&cache, routes).await);
    assert!(message.contains("streams more than"), "{message}");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_missing_asset_is_a_typed_failure() {
    let dir = scratch("install-404");
    let message = detail(download_into(&dir.join("cache"), HashMap::new()).await);
    assert!(message.contains("404"), "{message}");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn an_archive_whose_entry_is_a_symlink_is_refused() {
    let dir = scratch("install-symlink");
    let archive = archive_of(&dir, |source| {
        std::os::unix::fs::symlink("/etc/passwd", source.join(ANCHOR_BINARY)).unwrap();
    });
    let cache = dir.join("cache");
    let message =
        detail(download_into(&cache, routes_for(archive.clone(), checksum(&archive))).await);
    assert!(message.contains("not a regular file"), "{message}");
    assert!(!cache.join(ANCHOR_BINARY).exists());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn an_archive_without_the_binary_is_refused() {
    let dir = scratch("install-empty");
    let other = dir.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("README"), b"x").unwrap();
    let archive_path = dir.join("other.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&other)
        .arg("README")
        .status()
        .unwrap();
    assert!(status.success());
    let archive = std::fs::read(archive_path).unwrap();
    let cache = dir.join("cache");
    let message =
        detail(download_into(&cache, routes_for(archive.clone(), checksum(&archive))).await);
    assert!(message.contains("tar could not extract"), "{message}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn tar_is_resolved_on_absolute_path_entries_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("tar-path");
    let tar = dir.join("tar");
    std::fs::write(&tar, b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&tar, std::fs::Permissions::from_mode(0o755)).unwrap();
    let with_dir = std::env::join_paths([&dir]).unwrap();
    assert_eq!(tar_program(Some(&with_dir)), Ok(tar));
    assert!(tar_program(None).is_err());
    assert!(tar_program(Some(std::ffi::OsStr::new("."))).is_err());
    assert!(tar_program(Some(std::ffi::OsStr::new("bin:../tar"))).is_err());
    std::fs::remove_dir_all(dir).ok();
}
