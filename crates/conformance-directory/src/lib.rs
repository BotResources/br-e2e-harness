pub mod anchor;
pub mod build;
pub mod consumer;
pub mod error;
pub mod extensions;
pub mod harness;
pub mod kv_read;
pub mod outcome;
pub mod pg;
pub mod publish_fixture;
pub mod publisher;
pub mod source;
pub mod users_only;
pub mod wire;

pub use anchor::{DirectorySnapshotWire, KvEntry, build_and_emit, emit_snapshot};
pub use build::{build_anchor, ensure_go_available, subject_dir};
pub use consumer::{consumer_reads_groups, consumer_reads_users};
pub use error::{ConformanceError, Result};
pub use extensions::{
    MEMBERSHIP_FLAG, extension_survives_projection, filter_flip_orphan_deletes,
    reserved_key_rejected,
};
pub use harness::DirectoryHarness;
pub use kv_read::{read_groups, read_meta, read_users};
pub use outcome::{CheckId, CheckOutcome, CheckStatus, ConformanceReport};
pub use pg::ConsumerDb;
pub use publish_fixture::publish_snapshot;
pub use publisher::{publisher_floor, publisher_groups_optional};
pub use source::AnchorSource;
pub use users_only::users_only_narrows_projection;
pub use wire::{
    NEUTRAL_EXTENSION_KEY, deserialize_group, deserialize_meta, deserialize_user, run_wire_battery,
};
