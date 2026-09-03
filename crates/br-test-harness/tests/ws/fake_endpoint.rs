use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::any;
use br_test_harness::TestServer;

#[derive(Clone, Copy)]
pub enum Step {
    Next,
    ErrorFrame,
    Complete,
    Close,
    CloseWith(u16, &'static str),
    Garbage,
    Ping,
}

#[derive(Default)]
struct Observed {
    request_headers: HeaderMap,
    client_frames: Vec<String>,
}

pub struct FakeEndpoint {
    steps: Vec<Step>,
    observed: Mutex<Observed>,
}

impl FakeEndpoint {
    pub fn header(&self, name: &str) -> Option<String> {
        self.observed
            .lock()
            .unwrap()
            .request_headers
            .get(name)
            .map(|v| v.to_str().expect("header is not ascii").to_string())
    }

    pub fn client_frames(&self) -> Vec<String> {
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
            Step::CloseWith(code, reason) => Message::Close(Some(CloseFrame {
                code: *code,
                reason: (*reason).into(),
            })),
            Step::Garbage => Message::Text("not json".into()),
            Step::Ping => Message::Text(r#"{"type":"ping"}"#.into()),
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

pub async fn spawn(steps: Vec<Step>) -> (Arc<FakeEndpoint>, String) {
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
    (endpoint, server.base_url)
}
