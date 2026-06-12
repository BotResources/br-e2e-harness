use std::time::Duration;

use async_nats::jetstream::{self, kv};

pub struct TestNats {
    client: async_nats::Client,
    js: jetstream::Context,
    kv: kv::Store,
    bearer_kv: kv::Store,
    bucket_name: String,
    bearer_bucket_name: String,
}

impl TestNats {
    pub async fn setup() -> Self {
        Self::setup_on(&nats_url_from_env()).await
    }

    pub async fn setup_on(url: &str) -> Self {
        let client = connect(url)
            .await
            .expect("failed to connect to NATS — is `nats-server -js` running?");
        let js = jetstream::new(client.clone());

        let suffix = uuid::Uuid::now_v7().simple().to_string();
        let bucket_name = format!("test_{suffix}");
        let bearer_bucket_name = format!("test_bearer_{suffix}");
        let kv = create_bucket(&js, &bucket_name).await;
        let bearer_kv = create_bucket(&js, &bearer_bucket_name).await;

        Self {
            client,
            js,
            kv,
            bearer_kv,
            bucket_name,
            bearer_bucket_name,
        }
    }

    pub fn kv(&self) -> &kv::Store {
        &self.kv
    }

    pub fn bearer_kv(&self) -> &kv::Store {
        &self.bearer_kv
    }

    pub fn jetstream(&self) -> &jetstream::Context {
        &self.js
    }

    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }

    pub async fn create_kv(&self, bucket: &str) -> kv::Store {
        create_bucket(&self.js, bucket).await
    }

    pub async fn publish_raw(&self, subject: &str, payload: Vec<u8>) {
        self.client
            .publish(subject.to_string(), payload.into())
            .await
            .expect("nats publish failed");
        self.client.flush().await.expect("nats flush failed");
    }

    pub async fn cleanup(self) {
        for bucket in [&self.bucket_name, &self.bearer_bucket_name] {
            if let Err(e) = self.js.delete_key_value(bucket).await {
                eprintln!("warning: failed to delete test KV bucket '{bucket}': {e}");
            }
        }
    }
}

pub fn nats_url_from_env() -> String {
    dotenvy::dotenv().ok();
    std::env::var("NATS_URL").unwrap_or_else(|_| {
        panic!(
            "NATS_URL must be set to run tests (e.g. NATS_URL=nats://localhost:4222). \
             Ensure `nats-server -js` is running."
        )
    })
}

pub async fn connect(url: &str) -> Result<async_nats::Client, async_nats::ConnectError> {
    let mut options = async_nats::ConnectOptions::new().retry_on_initial_connect();
    if let Ok(parsed) = url::Url::parse(url) {
        let user = parsed.username();
        if !user.is_empty() {
            let pass = parsed.password().unwrap_or("");
            let user = percent_encoding::percent_decode_str(user)
                .decode_utf8_lossy()
                .into_owned();
            let pass = percent_encoding::percent_decode_str(pass)
                .decode_utf8_lossy()
                .into_owned();
            options = options.user_and_password(user, pass);
        }
    }
    options = options.connection_timeout(Duration::from_secs(10));
    options.connect(url).await
}

async fn create_bucket(js: &jetstream::Context, bucket: &str) -> kv::Store {
    js.create_key_value(kv::Config {
        bucket: bucket.to_string(),
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("failed to create test KV bucket '{bucket}': {e}"))
}
