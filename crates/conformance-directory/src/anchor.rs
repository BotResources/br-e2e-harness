use std::path::Path;
use std::time::Duration;

use br_core_directory::{META_KEY, group_id_from_kv_key, user_id_from_kv_key};
use br_test_harness::run_once;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirectorySnapshotWire {
    pub meta: KvEntry,
    pub users: Vec<KvEntry>,
    pub groups: Vec<KvEntry>,
}

impl DirectorySnapshotWire {
    pub fn user_id(entry: &KvEntry) -> Option<Uuid> {
        user_id_from_kv_key(&entry.key)
    }

    pub fn group_id(entry: &KvEntry) -> Option<Uuid> {
        group_id_from_kv_key(&entry.key)
    }
}

pub async fn emit_snapshot(binary: &Path) -> Result<DirectorySnapshotWire> {
    let output = run_once(&binary.to_string_lossy(), &[], &[], Duration::from_secs(60))
        .await
        .map_err(ConformanceError::Run)?;

    if !output.status.success() {
        return Err(ConformanceError::Run(format!(
            "the anchor exited with {} ; stderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let snapshot: DirectorySnapshotWire = serde_json::from_slice(&output.stdout).map_err(|e| {
        ConformanceError::Snapshot(format!("{e}\nstdout: {}", excerpt(&output.stdout)))
    })?;

    if snapshot.meta.key != META_KEY {
        return Err(ConformanceError::Snapshot(format!(
            "meta key {:?} != frozen META_KEY {META_KEY:?}",
            snapshot.meta.key
        )));
    }
    Ok(snapshot)
}

pub async fn build_and_emit() -> Result<DirectorySnapshotWire> {
    let binary = crate::build::build_anchor().await?;
    emit_snapshot(&binary).await
}

fn excerpt(bytes: &[u8]) -> String {
    const MAX: usize = 400;
    let text = String::from_utf8_lossy(bytes);
    if text.len() > MAX {
        format!("{}…", text.chars().take(MAX).collect::<String>())
    } else {
        text.into_owned()
    }
}
