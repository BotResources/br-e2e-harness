use br_util_directory::DirectoryPublisher;
use br_util_nats_fabric::Fabric;

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
