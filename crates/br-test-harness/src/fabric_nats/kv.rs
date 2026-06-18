use std::collections::BTreeMap;

use async_nats::jetstream::kv::Store;
use br_core_directory::{DirectoryMeta, META_KEY};
use futures_util::StreamExt as _;
use serde::de::DeserializeOwned;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum FabricKvError {
    #[error("published-language bucket access failed: {0}")]
    Bucket(String),
    #[error("key enumeration failed: {0}")]
    Keys(String),
    #[error("value on key '{key}' failed to deserialize: {detail}")]
    Decode { key: String, detail: String },
}

pub async fn pl_list<V: DeserializeOwned>(
    store: &Store,
    id_from_key: fn(&str) -> Option<Uuid>,
) -> Result<BTreeMap<Uuid, V>, FabricKvError> {
    let mut keys = store
        .keys()
        .await
        .map_err(|e| FabricKvError::Keys(e.to_string()))?;
    let mut entries = BTreeMap::new();
    while let Some(key) = keys.next().await {
        let key = key.map_err(|e| FabricKvError::Keys(e.to_string()))?;
        let Some(id) = id_from_key(&key) else {
            continue;
        };
        let Some(bytes) = store
            .get(&key)
            .await
            .map_err(|e| FabricKvError::Bucket(e.to_string()))?
        else {
            continue;
        };
        let value = serde_json::from_slice(&bytes).map_err(|e| FabricKvError::Decode {
            key: key.clone(),
            detail: e.to_string(),
        })?;
        entries.insert(id, value);
    }
    Ok(entries)
}

pub async fn pl_get_meta(store: &Store) -> Result<Option<DirectoryMeta>, FabricKvError> {
    let Some(bytes) = store
        .get(META_KEY)
        .await
        .map_err(|e| FabricKvError::Bucket(e.to_string()))?
    else {
        return Ok(None);
    };
    let meta = serde_json::from_slice(&bytes).map_err(|e| FabricKvError::Decode {
        key: META_KEY.to_string(),
        detail: e.to_string(),
    })?;
    Ok(Some(meta))
}
