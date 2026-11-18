use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::Instant;

pub struct ReadyzStandIn {
    pub url: String,
    task: tokio::task::JoinHandle<()>,
}

impl ReadyzStandIn {
    pub async fn start(fails_after: Option<Duration>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("http://{}/readyz", listener.local_addr().expect("addr"));
        let first_request = Arc::new(Mutex::new(None::<Instant>));
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let first_request = first_request.clone();
                tokio::spawn(async move {
                    let mut request = [0u8; 1024];
                    let _ = stream.read(&mut request).await;
                    let ready = {
                        let mut first = first_request.lock().unwrap();
                        let since = *first.get_or_insert_with(Instant::now);
                        fails_after.is_none_or(|after| since.elapsed() < after)
                    };
                    let status = if ready {
                        "200 OK"
                    } else {
                        "503 Service Unavailable"
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        Self { url, task }
    }
}

impl Drop for ReadyzStandIn {
    fn drop(&mut self) {
        self.task.abort();
    }
}
