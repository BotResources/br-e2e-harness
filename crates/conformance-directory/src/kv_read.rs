use std::collections::BTreeMap;

use br_core_directory::{
    DirectoryMeta, GROUPS_KEY_PREFIX, PublishedGroup, PublishedUser, USERS_KEY_PREFIX,
    group_id_from_kv_key, user_id_from_kv_key,
};
use br_test_harness::{FabricKvError, FabricTestNats};
use br_util_nats_fabric::KvPrefix;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub async fn read_users(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedUser>> {
    require_bucket(nats).await?;
    require_conformant_keys(nats, USERS_KEY_PREFIX, "PublishedUser", user_id_from_kv_key).await?;
    nats.pl_list::<PublishedUser>(user_id_from_kv_key)
        .await
        .map_err(|e| map_kv(e, "PublishedUser"))
}

pub async fn read_groups(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedGroup>> {
    require_bucket(nats).await?;
    require_conformant_keys(
        nats,
        GROUPS_KEY_PREFIX,
        "PublishedGroup",
        group_id_from_kv_key,
    )
    .await?;
    nats.pl_list::<PublishedGroup>(group_id_from_kv_key)
        .await
        .map_err(|e| map_kv(e, "PublishedGroup"))
}

pub async fn read_meta(nats: &FabricTestNats) -> Result<Option<DirectoryMeta>> {
    require_bucket(nats).await?;
    nats.pl_get_meta()
        .await
        .map_err(|e| map_kv(e, "DirectoryMeta"))
}

async fn require_bucket(nats: &FabricTestNats) -> Result<()> {
    if nats.published_language_present().await {
        Ok(())
    } else {
        Err(ConformanceError::Jetstream(
            "the PUBLISHED_LANGUAGE bucket is absent: the fabric-nats provisioning must create it \
             before the published directory is read"
                .to_string(),
        ))
    }
}

async fn require_conformant_keys(
    nats: &FabricTestNats,
    prefix: &str,
    ty: &'static str,
    id_from_key: fn(&str) -> Option<Uuid>,
) -> Result<()> {
    let prefix = KvPrefix::new(prefix).map_err(|e| ConformanceError::Directory(e.to_string()))?;
    let keys = nats
        .pl_reader::<serde_json::Value>()
        .await
        .keys(&prefix)
        .await
        .map_err(|e| ConformanceError::Jetstream(e.to_string()))?;
    match keys.iter().find(|key| id_from_key(key.as_str()).is_none()) {
        Some(key) => Err(ConformanceError::NonConformantWire {
            key: key.as_str().to_string(),
            ty,
            cause: format!(
                "the key sits under {} but is not `{}{{uuid}}`",
                prefix.as_str(),
                prefix.as_str()
            ),
        }),
        None => Ok(()),
    }
}

fn map_kv(error: FabricKvError, ty: &'static str) -> ConformanceError {
    match error {
        FabricKvError::Decode { key, detail } => ConformanceError::NonConformantWire {
            key,
            ty,
            cause: detail,
        },
        other => ConformanceError::Jetstream(other.to_string()),
    }
}
