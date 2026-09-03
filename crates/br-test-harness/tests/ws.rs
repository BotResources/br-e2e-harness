#![cfg(all(feature = "ws", feature = "server", feature = "passport"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::any;
use br_test_harness::{
    PassportBuilder, TestServer, WsCredential, WsError, WsSubscription, wait_until,
};

const PUSH: Duration = Duration::from_millis(500);
const QUERY: &str = "subscription { tick }";

#[derive(Clone, Copy)]
enum Step {
    Next,
    ErrorFrame,
    Complete,
    Close,
}

#[derive(Default)]
struct Observed {
    request_headers: HeaderMap,
    client_frames: Vec<String>,
}

struct FakeEndpoint {
    steps: Vec<Step>,
    observed: Mutex<Observed>,
}

impl FakeEndpoint {
    fn header(&self, name: &str) -> Option<String> {
        self.observed
            .lock()
            .unwrap()
            .request_headers
            .get(name)
            .map(|v| v.to_str().expect("header is not ascii").to_string())
    }

    fn client_frames(&self) -> Vec<String> {
        self.observed.lock().unwrap().client_frames.clone()
    }
}

async fn upgrade(
    State(endpoint): State<Arc<FakeEndpoint>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    endpoint.observed.lock().unwrap().request_headers = headers;
    ws.protocols(["graphql-transport-ws"])
        .on_upgrade(move |socket| drive(socket, endpoint))
}

async fn drive(mut socket: WebSocket, endpoint: Arc<FakeEndpoint>) {
    if !await_client_type(&mut socket, &endpoint, "connection_init").await {
        return;
    }
    if socket
        .send(Message::Text(r#"{"type":"connection_ack"}"#.into()))
        .await
        .is_err()
    {
        return;
    }
    if !await_client_type(&mut socket, &endpoint, "subscribe").await {
        return;
    }

    for step in &endpoint.steps {
        let frame = match step {
            Step::Next => {
                Message::Text(r#"{"id":"1","type":"next","payload":{"data":{"tick":7}}}"#.into())
            }
            Step::ErrorFrame => {
                Message::Text(r#"{"id":"1","type":"error","payload":[{"message":"boom"}]}"#.into())
            }
            Step::Complete => Message::Text(r#"{"id":"1","type":"complete"}"#.into()),
            Step::Close => Message::Close(None),
        };
        if socket.send(frame).await.is_err() {
            return;
        }
    }

    while let Some(Ok(frame)) = socket.recv().await {
        record(&endpoint, &frame);
    }
}

async fn await_client_type(
    socket: &mut WebSocket,
    endpoint: &Arc<FakeEndpoint>,
    want: &str,
) -> bool {
    while let Some(Ok(frame)) = socket.recv().await {
        record(endpoint, &frame);
        if let Message::Text(text) = &frame {
            let msg: serde_json::Value =
                serde_json::from_str(text.as_str()).expect("client frame is json");
            if msg["type"] == want {
                return true;
            }
        }
    }
    false
}

fn record(endpoint: &Arc<FakeEndpoint>, frame: &Message) {
    if let Message::Text(text) = frame {
        endpoint
            .observed
            .lock()
            .unwrap()
            .client_frames
            .push(text.to_string());
    }
}

async fn open(
    steps: Vec<Step>,
    credential: WsCredential<'_>,
) -> (WsSubscription, Arc<FakeEndpoint>) {
    let endpoint = Arc::new(FakeEndpoint {
        steps,
        observed: Mutex::new(Observed::default()),
    });
    let server = TestServer::spawn(
        Router::new()
            .route("/graphql/ws", any(upgrade))
            .with_state(endpoint.clone()),
    )
    .await;
    let subscription = WsSubscription::open_with(&server.base_url, credential, QUERY)
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

    assert!(
        endpoint.header("x-passport").is_some(),
        "a Passport credential must reach the server as X-Passport"
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
async fn a_server_close_is_a_closed_outcome() {
    assert_eq!(first_outcome(vec![Step::Close]).await, Err(WsError::Closed));
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
