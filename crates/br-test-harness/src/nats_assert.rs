use std::time::Duration;

use async_nats::jetstream::{self, consumer, kv, stream};
use futures_util::StreamExt as _;
use serde_json::Value;

pub async fn await_integration_event(
    js: &jetstream::Context,
    stream: &str,
    subject: &str,
    deadline: Duration,
) -> Option<Value> {
    let stream = js.get_stream(stream).await.ok()?;
    let consumer = stream
        .create_consumer(consumer::pull::Config {
            durable_name: None,
            deliver_policy: consumer::DeliverPolicy::All,
            ack_policy: consumer::AckPolicy::None,
            filter_subject: subject.to_string(),
            inactive_threshold: Duration::from_secs(60),
            ..Default::default()
        })
        .await
        .ok()?;
    let mut messages = consumer.messages().await.ok()?;

    let next = tokio::time::timeout(deadline, messages.next())
        .await
        .ok()??;
    let msg = next.ok()?;
    serde_json::from_slice::<Value>(&msg.payload).ok()
}

pub async fn recreate_stream(
    js: &jetstream::Context,
    name: &str,
    subjects: &[&str],
) -> stream::Stream {
    let _ = js.delete_stream(name).await;
    js.create_stream(stream::Config {
        name: name.to_string(),
        subjects: subjects.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("create stream {name}: {e}"))
}

pub async fn recreate_kv(js: &jetstream::Context, bucket: &str) -> kv::Store {
    let _ = js.delete_key_value(bucket).await;
    js.create_key_value(kv::Config {
        bucket: bucket.to_string(),
        history: 8,
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("create kv bucket {bucket}: {e}"))
}
