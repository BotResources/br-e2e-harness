use std::sync::Arc;
use std::time::Duration;

use br_test_harness::{WsCredential, WsError, WsSubscription};

use super::fake_endpoint::{FakeEndpoint, Step, spawn};

pub const PUSH: Duration = Duration::from_millis(500);
pub const QUERY: &str = "subscription { tick }";

pub async fn open(
    steps: Vec<Step>,
    credential: WsCredential<'_>,
) -> (WsSubscription, Arc<FakeEndpoint>) {
    let (endpoint, base_url) = spawn(steps).await;
    let subscription = WsSubscription::open_with(&base_url, credential, QUERY)
        .await
        .expect("the fake endpoint completes the graphql-transport-ws handshake");
    (subscription, endpoint)
}

pub async fn first_outcome(steps: Vec<Step>) -> Result<serde_json::Value, WsError> {
    let (mut subscription, _endpoint) = open(steps, WsCredential::Anonymous).await;
    subscription.next_data_outcome(PUSH).await
}

pub async fn first_error_string(steps: Vec<Step>) -> String {
    let (mut subscription, _endpoint) = open(steps, WsCredential::Anonymous).await;
    subscription
        .next_data(PUSH)
        .await
        .expect_err("the scripted endpoint never pushes a `next` frame")
}
