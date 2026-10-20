use std::path::{Path, PathBuf};

use br_test_harness::spawn_fabric_provision;

use crate::binary::cache_dir_from;
use crate::containment::contained;
use crate::error::{ConformanceError, Result};

const EMBEDDED_MANIFESTS: [(&str, &str); 1] = [(
    "published_language.toml",
    include_str!("../tests/fixtures/published_language.toml"),
)];

pub fn manifest_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub async fn provision(nats_url: &str, manifest: &str) -> Result<()> {
    crate::prerequisite::fabric_nats()?;
    let manifest = readable_manifest(manifest)?;
    contained(spawn_fabric_provision(nats_url, &manifest))
        .await
        .map_err(|panic| {
            ConformanceError::Jetstream(format!(
                "provisioning the fabric could not run (a spawnable `sh` is required): {panic}"
            ))
        })?
        .map_err(ConformanceError::Jetstream)
}

fn readable_manifest(name: &str) -> Result<PathBuf> {
    let source_tree = manifest_path(name);
    if source_tree.is_file() {
        return Ok(source_tree);
    }
    let Some((_, content)) = EMBEDDED_MANIFESTS.iter().find(|(known, _)| *known == name) else {
        return Ok(source_tree);
    };
    let cache = cache_dir_from(|var| std::env::var_os(var)).ok_or_else(|| {
        ConformanceError::Jetstream(
            "no cache directory to hold the provisioning manifest: XDG_CACHE_HOME and HOME are \
             not set to an absolute path"
                .to_string(),
        )
    })?;
    write_manifest(&cache.join("manifests"), name, content)
}

fn write_manifest(directory: &Path, name: &str, content: &str) -> Result<PathBuf> {
    let target = directory.join(name);
    if std::fs::read_to_string(&target).is_ok_and(|existing| existing == content) {
        return Ok(target);
    }
    let io_error = |action: &str, path: &Path, e: std::io::Error| {
        ConformanceError::Jetstream(format!(
            "{action} the provisioning manifest at {}: {e}",
            path.display()
        ))
    };
    std::fs::create_dir_all(directory)
        .map_err(|e| io_error("creating the directory of", directory, e))?;
    let staging = directory.join(format!("{name}.{}.staging", uuid::Uuid::now_v7().simple()));
    std::fs::write(&staging, content).map_err(|e| io_error("writing", &staging, e))?;
    std::fs::rename(&staging, &target).map_err(|e| {
        std::fs::remove_file(&staging).ok();
        io_error("installing", &target, e)
    })?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scratch::scratch;

    #[test]
    fn the_embedded_manifest_is_the_committed_fixture() {
        let committed = std::fs::read_to_string(manifest_path("published_language.toml")).unwrap();
        assert_eq!(EMBEDDED_MANIFESTS[0].1, committed);
    }

    #[test]
    fn the_manifest_is_written_where_the_source_tree_is_absent() {
        let directory = scratch("manifest-write").join("manifests");
        let path = write_manifest(&directory, "published_language.toml", "a = 1\n").unwrap();
        assert_eq!(path, directory.join("published_language.toml"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 1\n");
    }

    #[test]
    fn a_stale_manifest_is_replaced_and_no_staging_file_is_left() {
        let directory = scratch("manifest-stale").join("manifests");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("published_language.toml"), "stale").unwrap();
        let path = write_manifest(&directory, "published_language.toml", "fresh").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh");
        let entries = std::fs::read_dir(&directory).unwrap().count();
        assert_eq!(entries, 1);
    }

    #[test]
    fn an_unwritable_location_is_a_typed_error() {
        let blocker = scratch("manifest-blocked");
        let file = blocker.join("not-a-directory");
        std::fs::write(&file, b"").unwrap();
        match write_manifest(
            &file.join("manifests"),
            "published_language.toml",
            "a = 1\n",
        ) {
            Err(ConformanceError::Jetstream(detail)) => {
                assert!(detail.contains("provisioning manifest"), "{detail}");
            }
            other => panic!("expected a typed error: {other:?}"),
        }
    }

    #[test]
    fn the_source_tree_fixture_is_used_when_it_exists_and_nothing_is_written() {
        assert_eq!(
            readable_manifest("published_language.toml").unwrap(),
            manifest_path("published_language.toml")
        );
    }

    #[test]
    fn an_unknown_manifest_name_keeps_the_source_tree_path() {
        assert_eq!(
            readable_manifest("other.toml").unwrap(),
            manifest_path("other.toml")
        );
    }
}
