use std::time::Duration;

use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::consumer::pull::Stream as PullStream;
use futures_util::StreamExt as _;

use super::FabricTestNats;
use super::observe::FixedStream;

pub struct TappedDelivery {
    pub subject: String,
    pub payload: Vec<u8>,
    pub delivered_count: i64,
}

pub struct DurableTap {
    messages: PullStream,
}

impl FabricTestNats {
    pub async fn tap_durable(&self, stream: FixedStream, durable: &str) -> DurableTap {
        let name = stream.name();
        let js_stream =
            self.js.get_stream(name).await.unwrap_or_else(|e| {
                panic!("get fixed stream {name} to tap durable {durable}: {e}")
            });
        let consumer: PullConsumer = js_stream
            .get_consumer(durable)
            .await
            .unwrap_or_else(|e| panic!("tap an absent durable {durable} on {name}: {e}"));
        let messages = consumer
            .messages()
            .await
            .unwrap_or_else(|e| panic!("open the pull stream of durable {durable} on {name}: {e}"));
        DurableTap { messages }
    }
}

impl DurableTap {
    pub async fn next_within(&mut self, within: Duration) -> Option<TappedDelivery> {
        let received = tokio::time::timeout(within, self.messages.next())
            .await
            .ok()??;
        let message =
            received.unwrap_or_else(|e| panic!("tapped durable yielded a broken frame: {e}"));
        let info = message
            .info()
            .unwrap_or_else(|e| panic!("tapped frame carries no JetStream delivery info: {e}"));
        Some(TappedDelivery {
            subject: message.subject.as_str().to_string(),
            payload: message.payload.to_vec(),
            delivered_count: info.delivered,
        })
    }

    pub async fn deliveries_within(&mut self, within: Duration, cap: usize) -> Vec<TappedDelivery> {
        let mut seen = Vec::new();
        while seen.len() < cap {
            match self.next_within(within).await {
                Some(delivery) => seen.push(delivery),
                None => break,
            }
        }
        seen
    }

    pub async fn drain(self) {
        drop(self.messages);
    }
}
