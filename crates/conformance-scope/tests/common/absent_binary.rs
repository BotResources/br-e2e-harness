use std::path::PathBuf;

use conformance_scope::SpawnTarget;

pub fn absent_binary() -> SpawnTarget {
    SpawnTarget {
        binary: PathBuf::from("/nonexistent/scope-service"),
    }
}
