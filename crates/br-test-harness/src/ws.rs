use std::time::Duration;

use br_core_auth::{Passport, PassportHeader};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct WsSubscription {
    socket: Socket,
}

impl WsSubscription {
    pub async fn open(base: &str, passport: &Passport, query: &str) -> Result<Self, String> {
        Self::open_at(base, "/graphql/ws", passport, query).await
    }

    pub async fn open_at(
        base: &str,
        ws_path: &str,
        passport: &Passport,
        query: &str,
    ) -> Result<Self, String> {
        let ws_url = format!(
            "{}{ws_path}",
            base.replacen("https://", "wss://", 1)
                .replacen("http://", "ws://", 1)
        );

        let mut request = ws_url
            .clone()
            .into_client_request()
            .map_err(|e| format!("ws: build request: {e}"))?;
        let headers = request.headers_mut();
        headers.insert(
            "Sec-WebSocket-Protocol",
            "graphql-transport-ws"
                .parse()
                .map_err(|e| format!("ws: subprotocol header: {e}"))?,
        );
        headers.insert(
            "X-Passport",
            passport
                .to_header()
                .parse()
                .map_err(|e| format!("ws: X-Passport header: {e}"))?,
        );

        let (mut socket, _resp) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| format!("ws: connect {ws_url}: {e}"))?;

        socket
            .send(Message::Text(
                json!({ "type": "connection_init" }).to_string().into(),
            ))
            .await
            .map_err(|e| format!("ws: send connection_init: {e}"))?;
        Self::await_message_type(&mut socket, "connection_ack").await?;

        socket
            .send(Message::Text(
                json!({
                    "id": "1",
                    "type": "subscribe",
                    "payload": { "query": query },
                })
                .to_string()
                .into(),
            ))
            .await
            .map_err(|e| format!("ws: send subscribe: {e}"))?;

        Ok(Self { socket })
    }

    pub async fn next_data(&mut self, timeout: Duration) -> Result<Value, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        self.next_frame_data(deadline).await
    }

    pub async fn next_matching<F>(
        &mut self,
        mut predicate: F,
        timeout: Duration,
    ) -> Result<Value, String>
    where
        F: FnMut(&Value) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut skipped: Vec<Value> = Vec::new();
        loop {
            match self.next_frame_data(deadline).await {
                Ok(data) => {
                    if predicate(&data) {
                        return Ok(data);
                    }
                    skipped.push(data);
                }
                Err(e) => {
                    return Err(format!(
                        "ws: no matching `next` push within the bounded window ({e}); \
                         skipped {} non-matching frame(s): {}",
                        skipped.len(),
                        serde_json::to_string(&skipped).unwrap_or_else(|_| "<unprintable>".into())
                    ));
                }
            }
        }
    }

    async fn next_frame_data(&mut self, deadline: tokio::time::Instant) -> Result<Value, String> {
        loop {
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| "ws: timed out waiting for a `next` push".to_string())?;
            let frame = tokio::time::timeout(remaining, self.socket.next())
                .await
                .map_err(|_| "ws: timed out waiting for a `next` push".to_string())?
                .ok_or_else(|| "ws: socket closed before a `next` push".to_string())?
                .map_err(|e| format!("ws: read frame: {e}"))?;

            let text = match frame {
                Message::Text(t) => t.to_string(),
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(c) => return Err(format!("ws: server closed: {c:?}")),
                _ => continue,
            };
            let msg: Value = serde_json::from_str(&text)
                .map_err(|e| format!("ws: parse frame `{text}`: {e}"))?;
            match msg["type"].as_str() {
                Some("next") => return Ok(msg["payload"]["data"].clone()),
                Some("error") => return Err(format!("ws: subscription error frame: {msg}")),
                Some("complete") => {
                    return Err("ws: subscription completed before any push".to_string());
                }
                Some("ping") => {
                    self.socket
                        .send(Message::Text(json!({ "type": "pong" }).to_string().into()))
                        .await
                        .map_err(|e| format!("ws: send pong: {e}"))?;
                }
                _ => {}
            }
        }
    }

    async fn await_message_type(socket: &mut Socket, want: &str) -> Result<(), String> {
        let frame = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await
            .map_err(|_| format!("ws: timed out awaiting `{want}`"))?
            .ok_or_else(|| format!("ws: socket closed awaiting `{want}`"))?
            .map_err(|e| format!("ws: read awaiting `{want}`: {e}"))?;
        let Message::Text(text) = frame else {
            return Err(format!("ws: expected text `{want}`, got a non-text frame"));
        };
        let msg: Value = serde_json::from_str(&text)
            .map_err(|e| format!("ws: parse `{want}` frame `{text}`: {e}"))?;
        if msg["type"].as_str() == Some(want) {
            Ok(())
        } else {
            Err(format!("ws: expected `{want}`, got `{text}`"))
        }
    }
}
