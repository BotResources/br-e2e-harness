mod outcome;

use std::time::Duration;

use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::consumer::pull::{MessagesErrorKind, Stream as PullStream};
use futures_util::StreamExt as _;

use super::FabricTestNats;
use super::observe::FixedStream;

pub use outcome::{TapOutcome, TapStop};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TappedDelivery {
    pub subject: String,
    pub payload: Vec<u8>,
    pub delivered_count: i64,
}

pub struct DurableTap {
    messages: PullStream,
    stream: FixedStream,
    durable: String,
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
        DurableTap {
            messages,
            stream,
            durable: durable.to_string(),
        }
    }
}

impl DurableTap {
    pub async fn next_within(&mut self, within: Duration) -> TapOutcome {
        match tokio::time::timeout(within, self.messages.next()).await {
            Err(_) => TapOutcome::Timeout,
            Ok(None) => TapOutcome::Closed,
            Ok(Some(Err(e))) => match e.kind() {
                MessagesErrorKind::ConsumerDeleted | MessagesErrorKind::PushBasedConsumer => {
                    TapOutcome::Closed
                }
                MessagesErrorKind::MissingHeartbeat
                | MessagesErrorKind::Pull
                | MessagesErrorKind::NoResponders
                | MessagesErrorKind::Other => {
                    panic!("{}: the pull stream errored: {e}", self.observer())
                }
            },
            Ok(Some(Ok(message))) => {
                let info = message.info().unwrap_or_else(|e| {
                    panic!(
                        "{}: tapped frame carries no delivery info: {e}",
                        self.observer()
                    )
                });
                TapOutcome::Delivery(TappedDelivery {
                    subject: message.subject.as_str().to_string(),
                    payload: message.payload.to_vec(),
                    delivered_count: info.delivered,
                })
            }
        }
    }

    pub async fn deliveries_within(
        &mut self,
        within: Duration,
        cap: usize,
    ) -> (Vec<TappedDelivery>, TapStop) {
        let mut seen = Vec::new();
        while seen.len() < cap {
            match self.next_within(within).await {
                TapOutcome::Delivery(delivery) => seen.push(delivery),
                TapOutcome::Timeout => return (seen, TapStop::Timeout),
                TapOutcome::Closed => return (seen, TapStop::Closed),
            }
        }
        (seen, TapStop::Limit)
    }

    pub async fn expect_delivery(&mut self, what: &str, within: Duration) -> TappedDelivery {
        match self.next_within(within).await {
            TapOutcome::Delivery(delivery) => delivery,
            TapOutcome::Timeout => panic!(
                "expected a delivery ({what}), got Timeout: {} pulled nothing within {within:?}",
                self.observer()
            ),
            TapOutcome::Closed => panic!(
                "expected a delivery ({what}), got Closed: {} lost its consumer",
                self.observer()
            ),
        }
    }

    pub async fn expect_quiet(&mut self, what: &str, quiet: Duration) {
        match self.next_within(quiet).await {
            TapOutcome::Timeout => {}
            TapOutcome::Delivery(delivery) => panic!(
                "expected no delivery ({what}), got delivery #{} on {}",
                delivery.delivered_count, delivery.subject
            ),
            TapOutcome::Closed => panic!(
                "expected no delivery ({what}), got Closed: {} lost its consumer, which is not quiet",
                self.observer()
            ),
        }
    }

    pub fn close(self) {
        drop(self.messages);
    }

    fn observer(&self) -> String {
        format!("tap on durable {} of {}", self.durable, self.stream.name())
    }
}
