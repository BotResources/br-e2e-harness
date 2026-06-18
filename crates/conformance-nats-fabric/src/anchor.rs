use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;

use crate::error::{ConformanceError, Result};
use crate::wire::FrozenWire;

pub fn anchor_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance-subjects/nats-fabric")
        .canonicalize()
        .expect("nats-fabric anchor directory must exist")
}

pub async fn ensure_go_available() -> Result<()> {
    match run_once("go", &["version"], &[], Duration::from_secs(30)).await {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(ConformanceError::GoUnavailable(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )),
        Err(e) => Err(ConformanceError::GoUnavailable(e)),
    }
}

pub async fn run_dead_grammar_guard(dir: &Path) -> Result<()> {
    let dir_str = dir.to_string_lossy().into_owned();
    let output = run_once(
        "make",
        &["-C", &dir_str, "guard"],
        &[],
        Duration::from_secs(60),
    )
    .await
    .map_err(ConformanceError::Build)?;
    if !output.status.success() {
        return Err(ConformanceError::Build(format!(
            "dead-grammar guard failed for {} (status {}):\n{}\n{}",
            dir.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

pub async fn build_anchor() -> Result<PathBuf> {
    ensure_go_available().await?;
    let dir = anchor_dir();
    run_dead_grammar_guard(&dir).await?;
    let dir_str = dir.to_string_lossy().into_owned();
    let binary =
        std::env::temp_dir().join(format!("nats-fabric-{}", uuid::Uuid::now_v7().simple()));
    let binary_str = binary.to_string_lossy().into_owned();

    let output = run_once(
        "go",
        &["build", "-C", &dir_str, "-o", &binary_str, "."],
        &[],
        Duration::from_secs(300),
    )
    .await
    .map_err(ConformanceError::Build)?;

    if !output.status.success() {
        return Err(ConformanceError::Build(format!(
            "go build failed (status {}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if !binary.exists() {
        return Err(ConformanceError::Build(format!(
            "go build reported success but {} is missing",
            binary.display()
        )));
    }
    Ok(binary)
}

pub async fn frozen_wire() -> Result<FrozenWire> {
    let binary = build_anchor().await?;
    let binary_str = binary.to_string_lossy().into_owned();
    let output = run_once(&binary_str, &[], &[], Duration::from_secs(30))
        .await
        .map_err(ConformanceError::Build)?;

    if !output.status.success() {
        return Err(ConformanceError::Build(format!(
            "anchor render failed (status {}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let wire: FrozenWire = serde_json::from_slice(&output.stdout)
        .map_err(|e| ConformanceError::Anchor(format!("anchor stdout is not frozen wire: {e}")))?;
    Ok(wire)
}
