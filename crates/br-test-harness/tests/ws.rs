#![cfg(all(feature = "ws", feature = "server", feature = "passport"))]

#[path = "ws/fake_endpoint.rs"]
mod fake_endpoint;

use std::sync::Arc;
use std::time::Duration;

use br_core_auth::PassportHeader;
use br_test_harness::{PassportBuilder, WsCredential, WsError, WsSubscription, wait_until};
use fake_endpoint::{FakeEndpoint, Step, spawn};

const PUSH: Duration = Duration::from_millis(500);
const QUERY: &str = "subscription { tick }";

async fn open(
    steps: Vec<Step>,
    credential: WsCredential<'_>,
) -> (WsSubscription, Arc<FakeEndpoint>) {
    let (endpoint, base_url) = spawn(steps).await;
    let subscription = WsSubscription::open_with(&base_url, credential, QUERY)
        .await
        .expect("the fake endpoint completes the graphql-transport-ws handshake");
    (subscription, endpoint)
}

async fn first_outcome(steps: Vec<Step>) -> Result<serde_json::Value, WsError> {
    let (mut subscription, _endpoint) = open(steps, WsCredential::Anonymous).await;
    subscription.next_data_outcome(PUSH).await
}

async fn first_error_string(steps: Vec<Step>) -> String {
    let (mut subscription, _endpoint) = open(steps, WsCredential::Anonymous).await;
    subscription
        .next_data(PUSH)
        .await
        .expect_err("the scripted endpoint never pushes a `next` frame")
}

#[tokio::test]
async fn a_passport_credential_sends_the_passport_header_and_no_cookie() {
    let passport = PassportBuilder::new().build();

    let (_subscription, endpoint) = open(vec![], WsCredential::Passport(&passport)).await;

    assert_eq!(
        endpoint.header("x-passport"),
        Some(passport.to_header()),
        "a Passport credential must reach the server as the encoded X-Passport header"
    );
    assert_eq!(
        endpoint.header("cookie"),
        None,
        "a Passport credential must not forge a Cookie"
    );
}

#[tokio::test]
async fn a_cookie_credential_reaches_the_server_and_sends_no_passport() {
    let (_subscription, endpoint) = open(vec![], WsCredential::Cookie("br_session=abc123")).await;

    assert_eq!(
        endpoint.header("cookie").as_deref(),
        Some("br_session=abc123"),
        "the Cookie credential must reach the server verbatim"
    );
    assert_eq!(
        endpoint.header("x-passport"),
        None,
        "a cookie-authenticated client must present no X-Passport — the edge injects it"
    );
}

#[tokio::test]
async fn an_anonymous_credential_sends_neither_header() {
    let (_subscription, endpoint) = open(vec![], WsCredential::Anonymous).await;

    assert_eq!(endpoint.header("x-passport"), None);
    assert_eq!(endpoint.header("cookie"), None);
}

#[tokio::test]
async fn a_next_push_yields_its_data_payload() {
    let outcome = first_outcome(vec![Step::Next]).await;

    assert_eq!(
        outcome.expect("a `next` frame is a successful outcome")["tick"],
        7
    );
}

#[tokio::test]
async fn silence_until_the_deadline_is_a_timeout_outcome() {
    assert_eq!(first_outcome(vec![]).await, Err(WsError::Timeout));
}

#[tokio::test]
async fn a_close_frame_without_a_payload_is_a_closed_outcome() {
    assert_eq!(first_outcome(vec![Step::Close]).await, Err(WsError::Closed));
}

#[tokio::test]
async fn a_close_frame_surfaces_the_rejection_code_and_reason() {
    let outcome = first_outcome(vec![Step::CloseWith(4401, "unauthorized")]).await;

    assert_eq!(
        outcome,
        Err(WsError::ServerClosed {
            code: 4401,
            reason: "unauthorized".to_string(),
        }),
        "graphql-transport-ws encodes the rejection in the close code — it must survive"
    );
    assert_eq!(
        outcome.unwrap_err().to_string(),
        "ws: server closed: code=4401 reason=unauthorized"
    );
}

#[tokio::test]
async fn an_unparsable_frame_is_a_transport_failure() {
    let outcome = first_outcome(vec![Step::Garbage]).await;

    let Err(WsError::Transport(_)) = outcome else {
        panic!("an unparsable frame must surface as WsError::Transport, got {outcome:?}");
    };
    assert!(
        first_error_string(vec![Step::Garbage])
            .await
            .starts_with("ws: parse frame `not json`: "),
        "the transport failure keeps the pre-1.2.0 message"
    );
}

#[tokio::test]
async fn a_ping_frame_is_ponged_and_the_read_loop_continues() {
    let (mut subscription, endpoint) =
        open(vec![Step::Ping, Step::Next], WsCredential::Anonymous).await;

    let data = subscription
        .next_data_outcome(PUSH)
        .await
        .expect("a ping must not end the read loop");

    assert_eq!(data["tick"], 7);
    let ponged = wait_until(PUSH, || async {
        endpoint
            .client_frames()
            .iter()
            .any(|f| f.contains("\"type\":\"pong\""))
    })
    .await;
    assert!(
        ponged,
        "a `ping` frame must be answered with a `pong`, saw {:?}",
        endpoint.client_frames()
    );
}

#[tokio::test]
async fn a_complete_frame_before_any_push_is_a_completed_outcome() {
    assert_eq!(
        first_outcome(vec![Step::Complete]).await,
        Err(WsError::Completed)
    );
}

#[tokio::test]
async fn an_error_frame_carries_the_server_payload() {
    let outcome = first_outcome(vec![Step::ErrorFrame]).await;

    let Err(WsError::ErrorFrame(frame)) = outcome else {
        panic!("an `error` frame must surface as WsError::ErrorFrame, got {outcome:?}");
    };
    assert!(
        frame.contains("boom"),
        "the error frame must carry the server payload, got {frame}"
    );
}

#[tokio::test]
async fn next_data_renders_every_outcome_through_display() {
    assert_eq!(
        first_error_string(vec![]).await,
        "ws: timed out waiting for a `next` push"
    );
    assert_eq!(
        first_error_string(vec![Step::Close]).await,
        "ws: socket closed before a `next` push"
    );
    assert_eq!(
        first_error_string(vec![Step::Complete]).await,
        "ws: subscription completed before any push"
    );
    assert_eq!(
        first_error_string(vec![Step::CloseWith(4403, "forbidden")]).await,
        "ws: server closed: code=4403 reason=forbidden"
    );
    assert!(
        first_error_string(vec![Step::ErrorFrame])
            .await
            .starts_with("ws: subscription error frame: ")
    );
}

#[tokio::test]
async fn close_sends_a_complete_frame_for_the_open_subscription() {
    let (subscription, endpoint) = open(vec![], WsCredential::Anonymous).await;

    subscription.close().await.expect("close is best-effort");

    let saw_complete = wait_until(PUSH, || async {
        endpoint
            .client_frames()
            .iter()
            .any(|f| f.contains("\"type\":\"complete\"") && f.contains("\"id\":\"1\""))
    })
    .await;
    assert!(
        saw_complete,
        "close() must end the subscription with a `complete` frame, saw {:?}",
        endpoint.client_frames()
    );
}

#[tokio::test]
async fn close_is_ok_when_the_server_already_closed_the_socket() {
    let (mut subscription, _endpoint) = open(vec![Step::Close], WsCredential::Anonymous).await;
    assert_eq!(
        subscription.next_data_outcome(PUSH).await,
        Err(WsError::Closed)
    );

    assert_eq!(
        subscription.close().await,
        Ok(()),
        "closing an already-closed socket is the ordinary end of a test, not a failure"
    );
}

#[tokio::test]
async fn a_trailing_slash_on_the_base_url_still_resolves_the_ws_path() {
    let (_endpoint, base_url) = spawn(vec![Step::Next]).await;

    let mut subscription = WsSubscription::open_at_with(
        &format!("{base_url}/"),
        "/graphql/ws",
        WsCredential::Anonymous,
        QUERY,
    )
    .await
    .expect("a trailing slash on the base must not build a `//graphql/ws` path");

    assert_eq!(
        subscription
            .next_data_outcome(PUSH)
            .await
            .expect("the endpoint pushes on the resolved path")["tick"],
        7
    );
}
