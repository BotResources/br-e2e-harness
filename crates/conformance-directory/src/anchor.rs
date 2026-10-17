use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use br_core_directory::{
    META_KEY, group_id_from_kv_key, group_kv_key, user_id_from_kv_key, user_kv_key,
};
use br_test_harness::run_once;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::binary::{locate_anchor, path_with_directory_first};
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

    pub(crate) fn keyed_users(&self) -> Result<BTreeMap<Uuid, &KvEntry>> {
        keyed(
            &self.users,
            Self::user_id,
            user_kv_key,
            "PublishedUser",
            "users",
        )
    }

    pub(crate) fn keyed_groups(&self) -> Result<BTreeMap<Uuid, &KvEntry>> {
        keyed(
            &self.groups,
            Self::group_id,
            group_kv_key,
            "PublishedGroup",
            "groups",
        )
    }
}

fn keyed<'a>(
    entries: &'a [KvEntry],
    id_of: fn(&KvEntry) -> Option<Uuid>,
    key_of: fn(Uuid) -> String,
    ty: &'static str,
    namespace: &str,
) -> Result<BTreeMap<Uuid, &'a KvEntry>> {
    let mut keyed = BTreeMap::new();
    for entry in entries {
        let Some(id) = id_of(entry) else {
            return Err(ConformanceError::NonConformantWire {
                key: entry.key.clone(),
                ty,
                cause: format!("the key is not a {namespace}/{{uuid}} key"),
            });
        };
        let canonical = key_of(id);
        if entry.key != canonical {
            return Err(ConformanceError::NonConformantWire {
                key: entry.key.clone(),
                ty,
                cause: format!("the key is not the canonical form {canonical}"),
            });
        }
        if let Some(first) = keyed.insert(id, entry) {
            return Err(ConformanceError::NonConformantWire {
                key: entry.key.clone(),
                ty,
                cause: format!("the key resolves to {id}, already carried by {}", first.key),
            });
        }
    }
    Ok(keyed)
}

pub async fn emit_snapshot(binary: &Path) -> Result<DirectorySnapshotWire> {
    let path = match binary.parent().filter(|dir| dir.is_absolute()) {
        Some(directory) => Some(path_with_directory_first(
            directory,
            std::env::var_os("PATH").as_deref(),
        )?),
        None => None,
    };
    let envs: Vec<(&str, &str)> = path.iter().map(|path| ("PATH", path.as_str())).collect();
    let output = run_once(
        &binary.to_string_lossy(),
        &[],
        &envs,
        Duration::from_secs(60),
    )
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

pub async fn locate_and_emit() -> Result<DirectorySnapshotWire> {
    let binary = locate_anchor().await?;
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
