pub mod spawned_process;
pub mod wait;

#[cfg(feature = "nats-fabric")]
pub mod fabric_nats;
#[cfg(feature = "nats")]
pub mod nats;
#[cfg(feature = "nats")]
pub mod nats_assert;
#[cfg(feature = "spawned-nats")]
pub mod spawned_nats;

#[cfg(feature = "e2e-db")]
pub mod e2e_db;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "passport")]
pub mod passport;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "sse")]
pub mod sse;
#[cfg(feature = "graphql")]
pub mod verdict;
#[cfg(feature = "ws")]
pub mod ws;

pub use spawned_process::{
    BootOutcome, SpawnedProcess, run_once, spawn_fabric_provision, workspace_bin,
};
pub use wait::wait_until;

#[cfg(feature = "nats-fabric")]
pub use fabric_nats::{
    BEARER_BUCKET, BareFabricNats, BearerSeedError, BearerSeeder, CapturedMessage, CommandAwaiter,
    CommandCapture, EventCapture, FabricAwaiter, FabricKvError, FabricTestNats, Manifest,
    ManifestError, NatsBacking, Rendered, RenderedCommand, RenderedEvent, RunNamespace,
    SeededToken, WidenedDurable, unknown_bearer,
};
#[cfg(feature = "nats")]
pub use nats::TestNats;
#[cfg(feature = "nats")]
pub use nats_assert::{await_integration_event, recreate_kv, recreate_stream};
#[cfg(feature = "spawned-nats")]
pub use spawned_nats::SpawnedNats;

#[cfg(feature = "e2e-db")]
pub use e2e_db::E2eDatabase;
#[cfg(feature = "graphql")]
pub use graphql::GraphqlClient;
#[cfg(feature = "passport")]
pub use passport::PassportBuilder;
#[cfg(feature = "server")]
pub use server::TestServer;
#[cfg(feature = "sse")]
pub use sse::{DrainStop, SseOutcome, SseSubscription};
#[cfg(feature = "ws")]
pub use ws::WsSubscription;

#[cfg(feature = "oidc")]
pub use oidc_test_idp as oidc;
