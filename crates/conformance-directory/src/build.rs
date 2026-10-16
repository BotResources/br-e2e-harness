use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;

use crate::error::{ConformanceError, Result};

pub fn subject_dir() -> PathBuf {
    try_subject_dir().expect("identity-directory subject directory must exist")
}

pub fn try_subject_dir() -> Result<PathBuf> {
    subject_dir_under(Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn subject_dir_under(crate_dir: &Path) -> Result<PathBuf> {
    let dir = crate_dir.join("../../conformance-subjects/identity-directory");
    dir.canonicalize().map_err(|e| {
        ConformanceError::Build(format!(
            "the identity-directory anchor sources are missing at {}: {e}",
            dir.display()
        ))
    })
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

pub async fn build_anchor() -> Result<PathBuf> {
    ensure_go_available().await?;
    let dir = try_subject_dir()?;
    let dir_str = dir.to_string_lossy().into_owned();
    let binary = std::env::temp_dir().join(format!(
        "identity-directory-{}",
        uuid::Uuid::now_v7().simple()
    ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_anchor_directory_is_a_typed_build_error() {
        let absent =
            std::env::temp_dir().join(format!("no-anchor-{}", uuid::Uuid::now_v7().simple()));
        match subject_dir_under(&absent) {
            Err(ConformanceError::Build(detail)) => assert!(detail.contains("identity-directory")),
            other => panic!("expected a typed build error: {other:?}"),
        }
    }

    #[test]
    fn the_shipped_anchor_directory_resolves() {
        assert!(try_subject_dir().unwrap().join("main.go").exists());
    }
}
