use async_nats::jetstream::kv::Store;
use br_util_directory::DirectoryPublisher;

use crate::error::{ConformanceError, Result};
use crate::source::AnchorSource;

pub async fn publish_snapshot(store: &Store, source: &AnchorSource) -> Result<()> {
    DirectoryPublisher::new(store.clone())
        .reconcile(source)
        .await
        .map_err(|e| ConformanceError::Directory(format!("seed KV via publisher reconcile: {e}")))
}
