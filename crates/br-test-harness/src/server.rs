use std::time::Duration;

use axum::Router;

const READINESS_DEADLINE: Duration = Duration::from_secs(5);
const PROBE_INTERVAL: Duration = Duration::from_millis(10);

pub struct TestServer {
    pub base_url: String,
}

impl TestServer {
    pub async fn spawn(router: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind failed");
        let addr = listener.local_addr().expect("local_addr failed");
        let base_url = format!("http://{addr}");

        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("test server stopped serving");
        });

        let client = reqwest::Client::new();
        let answered = tokio::time::timeout(READINESS_DEADLINE, async {
            while client.get(&base_url).send().await.is_err() {
                tokio::time::sleep(PROBE_INTERVAL).await;
            }
        })
        .await;
        if answered.is_err() {
            panic!("test server at {base_url} did not answer within {READINESS_DEADLINE:?}");
        }
        Self { base_url }
    }
}
