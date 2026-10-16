use axum::Router;

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
        for _ in 0..50 {
            if client.get(&base_url).send().await.is_ok() {
                return Self { base_url };
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("test server at {base_url} did not answer within 500ms");
    }
}
