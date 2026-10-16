use std::collections::BTreeMap;

use br_core_directory::{
    DirectoryMeta, PublishedGroup, PublishedUser, group_id_from_kv_key, user_id_from_kv_key,
};
use br_test_harness::{FabricKvError, FabricTestNats};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub async fn read_users(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedUser>> {
    require_bucket(nats).await?;
    nats.pl_list::<PublishedUser>(user_id_from_kv_key)
        .await
        .map_err(|e| map_kv(e, "PublishedUser"))
}

pub async fn read_groups(nats: &FabricTestNats) -> Result<BTreeMap<Uuid, PublishedGroup>> {
    require_bucket(nats).await?;
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
