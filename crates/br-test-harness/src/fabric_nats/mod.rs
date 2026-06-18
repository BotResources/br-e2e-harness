mod bearer;
mod capture;
mod cli_manifest;
mod connect;
mod kv;
mod namespace;
mod negative;

pub use bearer::{BEARER_BUCKET, BearerSeedError, BearerSeeder, SeededToken, unknown_bearer};
pub use capture::{CapturedMessage, CommandAwaiter, CommandCapture, EventCapture, FabricAwaiter};
pub use cli_manifest::{Manifest, ManifestError, Rendered, RenderedCommand, RenderedEvent};
pub use connect::NatsBacking;
pub use kv::FabricKvError;
pub use namespace::RunNamespace;
pub use negative::{BareFabricNats, WidenedDurable};

use std::collections::BTreeMap;

use async_nats::jetstream::{self, consumer};
use br_core_directory::DirectoryMeta;
use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{
    Fabric, FabricError, INTEGRATION_CMD, INTEGRATION_EVT, KvKey, KvPrefix, PublishErrorKind,
    PublishedLanguagePublisher, PublishedLanguageReader, command_subject, event_subject,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::spawned_nats::SpawnedNats;

pub struct FabricTestNats {
    backing: NatsBacking,
    client: async_nats::Client,
    js: jetstream::Context,
    fabric: Fabric,
    ns: RunNamespace,
    bearer: Option<async_nats::jetstream::kv::Store>,
}

impl FabricTestNats {
    pub async fn start() -> Self {
        let nats = SpawnedNats::start().await;
        let url = nats.url();
        let client = connect_or_panic(&url).await;
        Self::from_client(NatsBacking::Owned(nats), client).await
    }

    pub async fn connect(existing_url: &str) -> Self {
        let client = connect_or_panic(existing_url).await;
        Self::from_client(
            NatsBacking::Attached {
                url: existing_url.to_string(),
            },
            client,
        )
        .await
    }

    async fn from_client(backing: NatsBacking, client: async_nats::Client) -> Self {
        let js = jetstream::new(client.clone());
        connect::get_or_create_fixed_stream(&js, INTEGRATION_CMD, "integration.cmd.>").await;
        connect::get_or_create_fixed_stream(&js, INTEGRATION_EVT, "integration.evt.>").await;
        let fabric = Fabric::new(js.clone());
        let ns = RunNamespace::mint();
        Self {
            backing,
            client,
            js,
            fabric,
            ns,
            bearer: None,
        }
    }

    pub async fn with_published_language(self) -> Self {
        connect::get_or_create_published_language(&self.js).await;
        self
    }

    pub async fn with_bearer_tokens(mut self) -> Self {
        let store = connect::get_or_create_bucket(&self.js, bearer::BEARER_BUCKET).await;
        self.bearer = Some(store);
        self
    }

    pub async fn with_command_durable(self, coords: &CommandCoords, durable_name: &str) -> Self {
        let subject = command_subject(coords);
        self.create_durable(INTEGRATION_CMD, &self.ns.durable(durable_name), &subject)
            .await;
        self
    }

    pub async fn with_event_durable(self, coords: &EventCoords, durable_name: &str) -> Self {
        let subject = event_subject(coords);
        self.create_durable(INTEGRATION_EVT, &self.ns.durable(durable_name), &subject)
            .await;
        self
    }

    pub async fn provision_command_durable(&self, coords: &CommandCoords, durable: &str) {
        self.create_durable(INTEGRATION_CMD, durable, &command_subject(coords))
            .await;
    }

    pub async fn provision_event_durable(&self, coords: &EventCoords, durable: &str) {
        self.create_durable(INTEGRATION_EVT, durable, &event_subject(coords))
            .await;
    }

    pub async fn with_widened_durable(
        self,
        stream_name: &'static str,
        durable_name: &str,
        widened_filter: &str,
    ) -> (Self, WidenedDurable) {
        let durable = self.ns.durable(durable_name);
        self.create_durable(stream_name, &durable, widened_filter)
            .await;
        let marker = WidenedDurable {
            stream: stream_name,
            durable,
        };
        (self, marker)
    }

    pub fn fabric(&self) -> &Fabric {
        &self.fabric
    }

    pub fn fabric_owned(&self) -> Fabric {
        self.fabric.clone()
    }

    pub async fn publish_event_envelope(&self, coords: &EventCoords, bytes: &[u8]) {
        let subject = event_subject(coords);
        self.client
            .publish(subject, bytes.to_vec().into())
            .await
            .expect("publish event envelope onto the fabric");
        self.client
            .flush()
            .await
            .expect("flush event envelope onto the fabric");
    }

    pub async fn fixed_streams_present(&self) -> bool {
        self.js.get_stream(INTEGRATION_CMD).await.is_ok()
            && self.js.get_stream(INTEGRATION_EVT).await.is_ok()
    }

    pub async fn pl_get_raw(&self, key: &KvKey) -> Option<Vec<u8>> {
        self.published_language_store()
            .await
            .get(key.as_str())
            .await
            .unwrap_or_else(|e| panic!("pl_get_raw '{}': {e}", key.as_str()))
            .map(|bytes| bytes.to_vec())
    }

    pub async fn published_language_present(&self) -> bool {
        self.js
            .get_key_value(br_util_nats_fabric::KV_PUBLISHED_LANGUAGE)
            .await
            .is_ok()
    }

    pub fn url(&self) -> String {
        self.backing.url()
    }

    pub fn durable(&self, logical: &str) -> String {
        self.ns.durable(logical)
    }

    pub fn key_prefix(&self) -> KvPrefix {
        self.ns.key_prefix()
    }

    pub fn correlation(&self) -> Uuid {
        self.ns.correlation()
    }

    pub async fn capture_events(&self, coords: &[&EventCoords]) -> EventCapture {
        capture::capture_events(&self.js, coords).await
    }

    pub async fn capture_commands(&self, coords: &[&CommandCoords]) -> CommandCapture {
        capture::capture_commands(&self.js, coords).await
    }

    pub async fn await_event(&self, coords: &EventCoords) -> FabricAwaiter {
        let inner = self
            .fabric
            .await_event(coords)
            .await
            .expect("await_event: binding INTEGRATION_EVT failed");
        FabricAwaiter::new(inner)
    }

    pub async fn await_command(&self, coords: &CommandCoords) -> CommandAwaiter {
        capture::await_command_consumer(&self.js, coords).await
    }

    pub async fn pl_publisher<V>(&self) -> PublishedLanguagePublisher<V>
    where
        V: Serialize + DeserializeOwned + PartialEq + Clone,
    {
        PublishedLanguagePublisher::open(&self.fabric)
            .await
            .expect("pl_publisher: open PUBLISHED_LANGUAGE (call with_published_language first)")
    }

    pub async fn pl_reader<V>(&self) -> PublishedLanguageReader<V>
    where
        V: DeserializeOwned,
    {
        PublishedLanguageReader::open(&self.fabric)
            .await
            .expect("pl_reader: open PUBLISHED_LANGUAGE (call with_published_language first)")
    }

    pub async fn pl_list<V: DeserializeOwned>(
        &self,
        id_from_key: fn(&str) -> Option<Uuid>,
    ) -> Result<BTreeMap<Uuid, V>, FabricKvError> {
        kv::pl_list(&self.published_language_store().await, id_from_key).await
    }

    pub async fn pl_get_meta(&self) -> Result<Option<DirectoryMeta>, FabricKvError> {
        kv::pl_get_meta(&self.published_language_store().await).await
    }

    pub async fn pl_put_raw(&self, key: &KvKey, bytes: &[u8]) {
        self.published_language_store()
            .await
            .put(key.as_str(), bytes.to_vec().into())
            .await
            .unwrap_or_else(|e| panic!("pl_put_raw '{}': {e}", key.as_str()));
    }

    pub fn bearer_seeder(&self) -> BearerSeeder {
        let store = self
            .bearer
            .clone()
            .expect("bearer_seeder: call with_bearer_tokens first");
        BearerSeeder { store }
    }

    pub async fn assert_missing_stream(&self, coords: &EventCoords, durable: &str) -> FabricError {
        negative::assert_missing_stream(&self.js, coords, durable).await
    }

    pub async fn publish_dead_subject(&self, subject: &str, bytes: &[u8]) -> PublishErrorKind {
        negative::publish_dead_subject(&self.js, subject, bytes).await
    }

    pub async fn raw_message_absent(&self, stream: &str, subject: &str) -> bool {
        negative::raw_message_absent(&self.js, stream, subject).await
    }

    pub async fn shutdown(self) {
        self.backing.shutdown().await;
    }

    async fn published_language_store(&self) -> async_nats::jetstream::kv::Store {
        self.js
            .get_key_value(br_util_nats_fabric::KV_PUBLISHED_LANGUAGE)
            .await
            .expect("published-language bucket (call with_published_language first)")
    }

    async fn create_durable(&self, stream_name: &'static str, durable: &str, filter: &str) {
        let stream = self.js.get_stream(stream_name).await.unwrap_or_else(|e| {
            panic!("fixed stream {stream_name} must exist before binding: {e}")
        });
        stream
            .create_consumer(consumer::pull::Config {
                durable_name: Some(durable.to_string()),
                filter_subjects: vec![filter.to_string()],
                ack_policy: consumer::AckPolicy::Explicit,
                ack_wait: std::time::Duration::from_secs(2),
                ..Default::default()
            })
            .await
            .unwrap_or_else(|e| {
                panic!("create durable {durable} on {stream_name} filtering {filter}: {e}")
            });
    }
}

async fn connect_or_panic(url: &str) -> async_nats::Client {
    crate::nats::connect(url).await.unwrap_or_else(|e| {
        panic!("failed to connect to NATS at {url} for the fabric harness: {e}")
    })
}
