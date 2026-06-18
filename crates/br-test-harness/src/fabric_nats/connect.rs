use async_nats::jetstream::{self, stream};
use br_util_nats_fabric::KV_PUBLISHED_LANGUAGE;

use crate::spawned_nats::SpawnedNats;

pub enum NatsBacking {
    Owned(SpawnedNats),
    Attached { url: String },
}

impl NatsBacking {
    pub fn url(&self) -> String {
        match self {
            NatsBacking::Owned(nats) => nats.url(),
            NatsBacking::Attached { url } => url.clone(),
        }
    }

    pub async fn shutdown(self) {
        match self {
            NatsBacking::Owned(nats) => nats.shutdown().await,
            NatsBacking::Attached { .. } => {}
        }
    }
}

pub async fn get_or_create_fixed_stream(js: &jetstream::Context, name: &'static str, bind: &str) {
    if js.get_stream(name).await.is_ok() {
        return;
    }
    js.create_stream(stream::Config {
        name: name.to_string(),
        subjects: vec![bind.to_string()],
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("get-or-create fixed fabric stream {name} binding {bind}: {e}"));
}

pub async fn get_or_create_published_language(js: &jetstream::Context) {
    if js.get_key_value(KV_PUBLISHED_LANGUAGE).await.is_ok() {
        return;
    }
    js.create_key_value(jetstream::kv::Config {
        bucket: KV_PUBLISHED_LANGUAGE.to_string(),
        history: 8,
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("get-or-create published-language bucket: {e}"));
}

pub async fn get_or_create_bucket(
    js: &jetstream::Context,
    bucket: &str,
) -> async_nats::jetstream::kv::Store {
    if let Ok(store) = js.get_key_value(bucket).await {
        return store;
    }
    js.create_key_value(jetstream::kv::Config {
        bucket: bucket.to_string(),
        history: 8,
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("get-or-create kv bucket {bucket}: {e}"))
}
