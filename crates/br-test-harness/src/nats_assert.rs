use std::time::Duration;

use async_nats::jetstream::{self, consumer, kv, stream};
use futures_util::StreamExt as _;
use serde_json::Value;

use crate::broker_answer::{bucket_not_found, delete_stream_not_found};

pub async fn await_integration_event(
    js: &jetstream::Context,
    stream: &str,
    subject: &str,
    deadline: Duration,
) -> Option<Value> {
    let stream = js
        .get_stream(stream)
        .await
        .unwrap_or_else(|e| panic!("await_integration_event: get stream {stream}: {e}"));
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
        .unwrap_or_else(|e| panic!("await_integration_event: create consumer on {subject}: {e}"));
    let mut messages = consumer.messages().await.unwrap_or_else(|e| {
        panic!("await_integration_event: open the pull stream on {subject}: {e}")
    });

    let message = match tokio::time::timeout(deadline, messages.next()).await {
        Err(_) => {
            crate::nats::assert_connected(&js.client());
            return None;
        }
        Ok(None) => panic!("await_integration_event: the pull stream on {subject} ended"),
        Ok(Some(Err(e))) => {
            panic!("await_integration_event: the pull stream on {subject} errored: {e}")
        }
        Ok(Some(Ok(message))) => message,
    };
    Some(
        serde_json::from_slice::<Value>(&message.payload).unwrap_or_else(|e| {
            panic!("await_integration_event: the payload on {subject} is not JSON: {e}")
        }),
    )
}

pub async fn recreate_stream(
    js: &jetstream::Context,
    name: &str,
    subjects: &[&str],
) -> stream::Stream {
    let config = stream::Config {
        name: name.to_string(),
        subjects: subjects.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    for attempt in 0..6 {
        match js.delete_stream(name).await {
            Ok(_) => {}
            Err(e) if delete_stream_not_found(&e) => {}
            Err(_) if attempt < 5 => {
                tokio::time::sleep(backoff(attempt)).await;
                continue;
            }
            Err(e) => panic!("delete stream {name}: the broker did not answer \"not found\": {e}"),
        }
        match js.create_stream(config.clone()).await {
            Ok(stream) => return stream,
            Err(_) if attempt < 5 => tokio::time::sleep(backoff(attempt)).await,
            Err(e) => panic!("create stream {name}: {e}"),
        }
    }
    unreachable!("recreate_stream loop returns or panics within the bounded attempts")
}

pub async fn recreate_kv(js: &jetstream::Context, bucket: &str) -> kv::Store {
    let config = kv::Config {
        bucket: bucket.to_string(),
        history: 8,
        ..Default::default()
    };
    for attempt in 0..6 {
        match js.delete_key_value(bucket).await {
            Ok(_) => {}
            Err(e) if bucket_not_found(&e) => {}
            Err(_) if attempt < 5 => {
                tokio::time::sleep(backoff(attempt)).await;
                continue;
            }
            Err(e) => {
                panic!("delete kv bucket {bucket}: the broker did not answer \"not found\": {e}")
            }
        }
        match js.create_key_value(config.clone()).await {
            Ok(store) => return store,
            Err(_) if attempt < 5 => tokio::time::sleep(backoff(attempt)).await,
            Err(e) => panic!("create kv bucket {bucket}: {e}"),
        }
    }
    unreachable!("recreate_kv loop returns or panics within the bounded attempts")
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(20 * u64::from(attempt + 1))
}
