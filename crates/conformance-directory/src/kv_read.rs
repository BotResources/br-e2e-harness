use std::collections::BTreeMap;

use async_nats::jetstream::kv::Store;
use br_core_directory::{
    DirectoryMeta, META_KEY, PublishedGroup, PublishedUser, group_id_from_kv_key,
    user_id_from_kv_key,
};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub async fn read_users(store: &Store) -> Result<BTreeMap<Uuid, PublishedUser>> {
    read_entries(store, user_id_from_kv_key).await
}

pub async fn read_groups(store: &Store) -> Result<BTreeMap<Uuid, PublishedGroup>> {
    read_entries(store, group_id_from_kv_key).await
}

pub async fn read_meta(store: &Store) -> Result<Option<DirectoryMeta>> {
    let Some(bytes) = store
        .get(META_KEY)
        .await
        .map_err(|e| ConformanceError::Jetstream(format!("get '{META_KEY}': {e}")))?
    else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| ConformanceError::NonConformantWire {
            key: META_KEY.to_string(),
            ty: "DirectoryMeta",
            cause: e.to_string(),
        })
}

async fn read_entries<T: DeserializeOwned>(
    store: &Store,
    id_from_key: fn(&str) -> Option<Uuid>,
) -> Result<BTreeMap<Uuid, T>> {
    let mut keys = store
        .keys()
        .await
        .map_err(|e| ConformanceError::Jetstream(format!("list keys: {e}")))?;
    let mut entries = BTreeMap::new();
    while let Some(key) = keys.next().await {
        let key = key.map_err(|e| ConformanceError::Jetstream(format!("read key: {e}")))?;
        let Some(id) = id_from_key(&key) else {
            continue;
        };
        let Some(bytes) = store
            .get(&key)
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("get '{key}': {e}")))?
        else {
            continue;
        };
        let value =
            serde_json::from_slice(&bytes).map_err(|e| ConformanceError::NonConformantWire {
                key: key.clone(),
                ty: std::any::type_name::<T>(),
                cause: e.to_string(),
            })?;
        entries.insert(id, value);
    }
    Ok(entries)
}
