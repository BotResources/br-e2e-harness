use async_nats::jetstream;
use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{
    Fabric, FabricError, INTEGRATION_CMD, INTEGRATION_EVT, KV_PUBLISHED_LANGUAGE, PublishErrorKind,
};

use super::FabricTestNats;
use crate::nats::connect;
use crate::spawned_nats::SpawnedNats;

impl FabricTestNats {
    pub async fn assert_missing_stream(&self, coords: &EventCoords, durable: &str) -> FabricError {
        assert_missing_stream(&self.js, coords, durable).await
    }

    pub async fn publish_dead_subject(&self, subject: &str, bytes: &[u8]) -> PublishErrorKind {
        publish_dead_subject(&self.js, subject, bytes).await
    }

    pub async fn raw_message_absent(&self, stream: &str, subject: &str) -> bool {
        raw_message_absent(&self.js, stream, subject).await
    }

    pub async fn durable_filter_subjects(&self, stream: &str, durable: &str) -> Vec<String> {
        durable_filter_subjects(&self.js, stream, durable).await
    }

    pub async fn durable_filter_subjects_if_present(
        &self,
        stream: &str,
        durable: &str,
    ) -> Option<Vec<String>> {
        durable_filter_subjects_if_present(&self.js, stream, durable).await
    }
}

pub struct WidenedDurable {
    pub stream: &'static str,
    pub durable: String,
}

pub struct BareFabricNats {
    nats: SpawnedNats,
    js: jetstream::Context,
}

impl BareFabricNats {
    pub async fn without_fixed_streams() -> Self {
        let nats = SpawnedNats::start().await;
        let client = connect(&nats.url())
            .await
            .expect("connect to the bare spawned NATS");
        let js = jetstream::new(client);
        Self { nats, js }
    }

    pub async fn with_only_command_stream() -> Self {
        let this = Self::without_fixed_streams().await;
        this.create_stream(INTEGRATION_CMD, "integration.cmd.>")
            .await;
        this
    }

    pub async fn with_only_event_stream() -> Self {
        let this = Self::without_fixed_streams().await;
        this.create_stream(INTEGRATION_EVT, "integration.evt.>")
            .await;
        this
    }

    pub fn url(&self) -> String {
        self.nats.url()
    }

    pub async fn published_language_absent(&self) -> bool {
        self.js.get_key_value(KV_PUBLISHED_LANGUAGE).await.is_err()
    }

    pub async fn command_stream_absent(&self) -> bool {
        self.js.get_stream(INTEGRATION_CMD).await.is_err()
    }

    pub async fn event_stream_absent(&self) -> bool {
        self.js.get_stream(INTEGRATION_EVT).await.is_err()
    }

    pub async fn assert_missing_stream(&self, coords: &EventCoords, durable: &str) -> FabricError {
        assert_missing_stream(&self.js, coords, durable).await
    }

    pub async fn assert_missing_command_stream(
        &self,
        coords: &CommandCoords,
        durable: &str,
    ) -> FabricError {
        self.fabric()
            .verify_command_durable(coords, durable)
            .await
            .expect_err(
                "probing a command coordinate against a missing fixed stream must fail loud",
            )
    }

    pub async fn assert_missing_stream_on_bind(
        &self,
        coords: &EventCoords,
        durable: &str,
    ) -> FabricError {
        self.fabric()
            .ensure_event_durable(coords, durable)
            .await
            .expect_err(
                "binding an event durable against a missing fixed stream must fail loud, never create it",
            )
    }

    pub async fn assert_missing_command_stream_on_bind(
        &self,
        coords: &CommandCoords,
        durable: &str,
    ) -> FabricError {
        self.fabric()
            .ensure_command_durable(coords, durable)
            .await
            .expect_err(
                "binding a command durable against a missing fixed stream must fail loud, never create it",
            )
    }

    fn fabric(&self) -> Fabric {
        Fabric::new(self.js.clone())
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }

    async fn create_stream(&self, name: &'static str, bind: &str) {
        self.js
            .create_stream(jetstream::stream::Config {
                name: name.to_string(),
                subjects: vec![bind.to_string()],
                ..Default::default()
            })
            .await
            .unwrap_or_else(|e| panic!("create stream {name}: {e}"));
    }
}

pub async fn assert_missing_stream(
    js: &jetstream::Context,
    coords: &EventCoords,
    durable: &str,
) -> FabricError {
    let fabric = Fabric::new(js.clone());
    fabric
        .verify_event_durable(coords, durable)
        .await
        .expect_err("probing an event coordinate against a missing fixed stream must fail loud")
}

pub async fn publish_dead_subject(
    js: &jetstream::Context,
    subject: &str,
    bytes: &[u8],
) -> PublishErrorKind {
    let publish = js.publish(subject.to_string(), bytes.to_vec().into()).await;
    let err = match publish {
        Ok(ack) => ack
            .await
            .expect_err("a publish to the dead grammar must not be acked by any fixed stream"),
        Err(err) => err,
    };
    err.kind().into()
}

pub async fn raw_message_absent(js: &jetstream::Context, stream: &str, subject: &str) -> bool {
    let Ok(stream) = js.get_stream(stream).await else {
        return true;
    };
    stream
        .get_last_raw_message_by_subject(subject)
        .await
        .is_err()
}

pub async fn durable_filter_subjects(
    js: &jetstream::Context,
    stream_name: &str,
    durable: &str,
) -> Vec<String> {
    let stream = js
        .get_stream(stream_name)
        .await
        .unwrap_or_else(|e| panic!("get stream {stream_name} to read durable {durable}: {e}"));
    let mut consumer: jetstream::consumer::PullConsumer = stream
        .get_consumer(durable)
        .await
        .unwrap_or_else(|e| panic!("get durable {durable} on {stream_name}: {e}"));
    let config = &consumer
        .info()
        .await
        .unwrap_or_else(|e| panic!("read durable {durable} info on {stream_name}: {e}"))
        .config;
    filters_of(config)
}

pub async fn durable_filter_subjects_if_present(
    js: &jetstream::Context,
    stream_name: &str,
    durable: &str,
) -> Option<Vec<String>> {
    let stream = js.get_stream(stream_name).await.ok()?;
    let mut consumer: jetstream::consumer::PullConsumer =
        stream.get_consumer(durable).await.ok()?;
    Some(filters_of(&consumer.info().await.ok()?.config))
}

fn filters_of(config: &jetstream::consumer::Config) -> Vec<String> {
    if !config.filter_subjects.is_empty() {
        config.filter_subjects.clone()
    } else if !config.filter_subject.is_empty() {
        vec![config.filter_subject.clone()]
    } else {
        Vec::new()
    }
}
