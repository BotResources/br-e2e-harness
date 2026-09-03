mod group;
mod mirror;
mod user;

use std::sync::Arc;

use br_util_directory::{DirectoryError, DirectoryProjector};

use crate::harness::DirectoryHarness;
use crate::pg::ConsumerDb;
use crate::stager::recording::RecordingStager;

pub(crate) use group::{group_deleted, group_renamed, member_dropped};
pub(crate) use mirror::{boot, idempotence, without_stager};
pub(crate) use user::{rollback, user_deleted, user_upsert};

pub(crate) async fn reconcile_with(
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    stager: RecordingStager,
) -> Result<(), DirectoryError> {
    DirectoryProjector::new(harness.fabric().clone(), db.pool().clone())
        .with_impact_stager(Arc::new(stager))
        .reconcile()
        .await
        .map(|_| ())
}
