use std::collections::BTreeMap;
use std::future::Future;

use br_core_directory::{
    DirectoryMeta, GROUPS_KEY_PREFIX, PublishedGroup, PublishedUser, USERS_KEY_PREFIX,
    group_id_from_kv_key, group_kv_key, user_id_from_kv_key, user_kv_key,
};
use br_test_harness::{FabricKvError, FabricTestNats};
use br_util_nats_fabric::KvPrefix;
use uuid::Uuid;

use crate::containment::contained;
use crate::error::{ConformanceError, Result};

pub async fn read_users(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedUser>> {
    guarded(users(nats)).await
}

pub async fn read_groups(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedGroup>> {
    guarded(groups(nats)).await
}

pub async fn read_meta(nats: &FabricTestNats) -> Result<Option<DirectoryMeta>> {
    guarded(meta(nats)).await
}

async fn guarded<T>(read: impl Future<Output = Result<T>>) -> Result<T> {
    contained(read).await.unwrap_or_else(|panic| {
        Err(ConformanceError::Jetstream(format!(
            "the PUBLISHED_LANGUAGE bucket cannot be read, is the NATS server reachable? {panic}"
        )))
    })
}

async fn users(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedUser>> {
    require_bucket(nats).await?;
    require_conformant_keys(
        nats,
        USERS_KEY_PREFIX,
        "PublishedUser",
        user_id_from_kv_key,
        user_kv_key,
    )
    .await?;
    nats.pl_list::<PublishedUser>(user_id_from_kv_key)
        .await
        .map_err(|e| map_kv(e, "PublishedUser"))
}

async fn groups(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedGroup>> {
    require_bucket(nats).await?;
    require_conformant_keys(
        nats,
        GROUPS_KEY_PREFIX,
        "PublishedGroup",
        group_id_from_kv_key,
        group_kv_key,
    )
    .await?;
    nats.pl_list::<PublishedGroup>(group_id_from_kv_key)
        .await
        .map_err(|e| map_kv(e, "PublishedGroup"))
}

async fn meta(nats: &FabricTestNats) -> Result<Option<DirectoryMeta>> {
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
    key_of: fn(Uuid) -> String,
) -> Result<()> {
    let prefix = KvPrefix::new(prefix).map_err(|e| ConformanceError::Directory(e.to_string()))?;
    let keys = nats
        .pl_reader::<serde_json::Value>()
        .await
        .keys(&prefix)
        .await
        .map_err(|e| ConformanceError::Jetstream(e.to_string()))?;
    match keys
        .iter()
        .find(|key| !is_canonical(key.as_str(), id_from_key, key_of))
    {
        Some(key) => Err(ConformanceError::NonConformantWire {
            key: key.as_str().to_string(),
            ty,
            cause: format!(
                "the key sits under {} but is not the canonical `{}{{uuid}}` (hyphenated lowercase)",
                prefix.as_str(),
                prefix.as_str()
            ),
        }),
        None => Ok(()),
    }
}

fn is_canonical(
    key: &str,
    id_from_key: fn(&str) -> Option<Uuid>,
    key_of: fn(Uuid) -> String,
) -> bool {
    id_from_key(key).is_some_and(|id| key_of(id) == key)
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

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "01938c1f-0000-7000-8000-000000000001";

    #[test]
    fn only_the_hyphenated_lowercase_key_is_canonical() {
        let canonical = format!("{USERS_KEY_PREFIX}{ID}");
        assert!(is_canonical(&canonical, user_id_from_kv_key, user_kv_key));
        for spelling in [
            ID.to_uppercase(),
            ID.replace('-', ""),
            format!("{{{ID}}}"),
            format!("urn:uuid:{ID}"),
        ] {
            let key = format!("{USERS_KEY_PREFIX}{spelling}");
            assert!(
                !is_canonical(&key, user_id_from_kv_key, user_kv_key),
                "{key}"
            );
        }
    }

    #[test]
    fn a_group_key_is_canonical_only_in_its_own_namespace() {
        let key = format!("{GROUPS_KEY_PREFIX}{ID}");
        assert!(is_canonical(&key, group_id_from_kv_key, group_kv_key));
        assert!(!is_canonical(&key, user_id_from_kv_key, user_kv_key));
    }
}
