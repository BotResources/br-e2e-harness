use br_core_directory::PublishedUser;
use br_util_directory::DirectoryPublisher;
use br_util_nats_fabric::Fabric;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::source::AnchorSource;

pub async fn publish_snapshot(fabric: &Fabric, source: &AnchorSource) -> Result<()> {
    DirectoryPublisher::open(fabric)
        .await
        .map_err(|e| ConformanceError::Directory(format!("open publisher: {e}")))?
        .reconcile(source)
        .await
        .map_err(|e| ConformanceError::Directory(format!("seed KV via publisher reconcile: {e}")))
}

pub async fn publish_added_user(
    fabric: &Fabric,
    source: &AnchorSource,
    user_id: Uuid,
    user: PublishedUser,
) -> Result<()> {
    let mut next = source.clone();
    next.upsert_user(user_id, user);
    DirectoryPublisher::open(fabric)
        .await
        .map_err(|e| ConformanceError::Directory(format!("open publisher: {e}")))?
        .reconcile(&next)
        .await
        .map_err(|e| ConformanceError::Directory(format!("publish live user PUT: {e}")))
}
