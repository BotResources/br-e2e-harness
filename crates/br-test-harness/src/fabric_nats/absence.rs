use async_nats::jetstream::consumer;
use async_nats::jetstream::context::{GetStreamError, GetStreamErrorKind, KeyValueError};
use async_nats::jetstream::stream::{LastRawMessageErrorKind, Stream};
use async_nats::jetstream::{self, ErrorCode};

pub(super) fn stream_not_found(err: &GetStreamError) -> bool {
    matches!(err.kind(), GetStreamErrorKind::JetStream(inner) if inner.error_code() == ErrorCode::STREAM_NOT_FOUND)
}

fn bucket_not_found(err: &KeyValueError) -> bool {
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        if let Some(get) = cause.downcast_ref::<GetStreamError>() {
            return stream_not_found(get);
        }
        source = cause.source();
    }
    false
}

pub(super) async fn stream_if_present(js: &jetstream::Context, name: &str) -> Option<Stream> {
    match js.get_stream(name).await {
        Ok(stream) => Some(stream),
        Err(e) if stream_not_found(&e) => None,
        Err(e) => panic!("probe stream {name}: the broker did not answer \"absent\": {e}"),
    }
}

pub(super) async fn bucket_is_present(js: &jetstream::Context, bucket: &str) -> bool {
    match js.get_key_value(bucket).await {
        Ok(_) => true,
        Err(e) if bucket_not_found(&e) => false,
        Err(e) => panic!("probe kv bucket {bucket}: the broker did not answer \"absent\": {e}"),
    }
}

pub(super) async fn last_raw_message_is_absent(stream: &Stream, subject: &str) -> bool {
    match stream.get_last_raw_message_by_subject(subject).await {
        Ok(_) => false,
        Err(e) if matches!(e.kind(), LastRawMessageErrorKind::NoMessageFound) => true,
        Err(e) => panic!(
            "probe the last message of {subject} on {}: the broker did not answer \"no message\": {e}",
            stream.cached_info().config.name
        ),
    }
}

pub(super) async fn durable_config_if_present(
    stream: &Stream,
    durable: &str,
) -> Option<consumer::Config> {
    use async_nats::jetstream::context::ConsumerInfoErrorKind;
    match stream.consumer_info(durable).await {
        Ok(info) => Some(info.config),
        Err(e) if matches!(e.kind(), ConsumerInfoErrorKind::NotFound) => None,
        Err(e) => panic!(
            "probe durable {durable} on {}: the broker did not answer \"absent\": {e}",
            stream.cached_info().config.name
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_nats::jetstream::{self, consumer, stream};

    use super::*;
    use crate::SpawnedNats;

    struct Dead {
        js: jetstream::Context,
    }

    async fn dead_broker() -> Dead {
        let server = SpawnedNats::start().await;
        let client = crate::nats::connect(&server.url())
            .await
            .expect("connect to the spawned server");
        let js = jetstream::new(client);
        server.shutdown().await;
        Dead { js }
    }

    async fn live_broker() -> (SpawnedNats, jetstream::Context) {
        let server = SpawnedNats::start().await;
        let client = crate::nats::connect(&server.url())
            .await
            .expect("connect to the spawned server");
        (server, jetstream::new(client))
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn a_missing_stream_and_a_missing_bucket_read_as_absent() {
        let (server, js) = live_broker().await;

        assert!(stream_if_present(&js, "NOPE").await.is_none());
        assert!(!bucket_is_present(&js, "nope").await);

        server.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn a_present_stream_bucket_message_and_durable_read_as_present() {
        let (server, js) = live_broker().await;
        let created = js
            .create_stream(stream::Config {
                name: "THINGS".to_string(),
                subjects: vec!["things.>".to_string()],
                ..Default::default()
            })
            .await
            .expect("create the stream");
        js.create_key_value(jetstream::kv::Config {
            bucket: "things".to_string(),
            ..Default::default()
        })
        .await
        .expect("create the bucket");

        let stream = stream_if_present(&js, "THINGS")
            .await
            .expect("the stream exists");
        assert!(bucket_is_present(&js, "things").await);
        assert!(last_raw_message_is_absent(&stream, "things.a").await);
        assert!(durable_config_if_present(&stream, "worker").await.is_none());

        js.publish("things.a", "x".into())
            .await
            .expect("publish")
            .await
            .expect("ack");
        created
            .create_consumer(consumer::pull::Config {
                durable_name: Some("worker".to_string()),
                ..Default::default()
            })
            .await
            .expect("create the durable");
        let stream = stream_if_present(&js, "THINGS")
            .await
            .expect("the stream exists");
        assert!(!last_raw_message_is_absent(&stream, "things.a").await);
        assert!(durable_config_if_present(&stream, "worker").await.is_some());

        server.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    #[should_panic(expected = "probe stream THINGS")]
    async fn a_dead_broker_does_not_read_as_an_absent_stream() {
        let dead = dead_broker().await;
        let _ = tokio::time::timeout(
            Duration::from_secs(30),
            stream_if_present(&dead.js, "THINGS"),
        )
        .await
        .expect("the probe answers within 30s");
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    #[should_panic(expected = "probe kv bucket things")]
    async fn a_dead_broker_does_not_read_as_an_absent_bucket() {
        let dead = dead_broker().await;
        let _ = tokio::time::timeout(
            Duration::from_secs(30),
            bucket_is_present(&dead.js, "things"),
        )
        .await
        .expect("the probe answers within 30s");
    }
}
