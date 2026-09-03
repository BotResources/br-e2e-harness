use async_nats::jetstream;
use br_util_nats_fabric::{INTEGRATION_CMD, INTEGRATION_EVT};

use super::FabricTestNats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStream {
    Cmd,
    Evt,
}

impl FixedStream {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cmd => INTEGRATION_CMD,
            Self::Evt => INTEGRATION_EVT,
        }
    }
}

impl FabricTestNats {
    pub async fn consumer_pending(&self, stream: FixedStream, durable: &str) -> u64 {
        self.consumer_info(stream, durable).await.num_pending
    }

    pub async fn consumer_delivered(&self, stream: FixedStream, durable: &str) -> u64 {
        self.consumer_info(stream, durable)
            .await
            .delivered
            .consumer_sequence
    }

    pub async fn consumer_redelivered(&self, stream: FixedStream, durable: &str) -> u64 {
        self.consumer_info(stream, durable).await.num_redelivered as u64
    }

    pub async fn command_stream_len(&self) -> u64 {
        self.stream_len(FixedStream::Cmd).await
    }

    pub async fn event_stream_len(&self) -> u64 {
        self.stream_len(FixedStream::Evt).await
    }

    pub async fn stream_len(&self, stream: FixedStream) -> u64 {
        let name = stream.name();
        self.js
            .get_stream(name)
            .await
            .unwrap_or_else(|e| panic!("get fixed stream {name} to read its message count: {e}"))
            .info()
            .await
            .unwrap_or_else(|e| panic!("read fixed stream {name} info: {e}"))
            .state
            .messages
    }

    async fn consumer_info(&self, stream: FixedStream, durable: &str) -> jetstream::consumer::Info {
        let name = stream.name();
        let js_stream =
            self.js.get_stream(name).await.unwrap_or_else(|e| {
                panic!("get fixed stream {name} to read durable {durable}: {e}")
            });
        let mut consumer: jetstream::consumer::PullConsumer = js_stream
            .get_consumer(durable)
            .await
            .unwrap_or_else(|e| panic!("get durable {durable} on {name}: {e}"));
        consumer
            .info()
            .await
            .unwrap_or_else(|e| panic!("read durable {durable} info on {name}: {e}"))
            .clone()
    }
}
