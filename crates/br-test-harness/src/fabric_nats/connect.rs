use async_nats::jetstream::context::{
    CreateKeyValueError, CreateStreamError, CreateStreamErrorKind,
};
use async_nats::jetstream::{self, ErrorCode, stream};
use br_util_nats_fabric::KV_PUBLISHED_LANGUAGE;

use crate::spawned_nats::SpawnedNats;

fn stream_already_exists(err: &CreateStreamError) -> bool {
    matches!(err.kind(), CreateStreamErrorKind::JetStream(inner) if inner.error_code() == ErrorCode::STREAM_NAME_EXIST)
}

fn bucket_already_exists(err: &CreateKeyValueError) -> bool {
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        if let Some(create) = cause.downcast_ref::<CreateStreamError>() {
            return stream_already_exists(create);
        }
        source = cause.source();
    }
    false
}

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
    match js
        .create_stream(stream::Config {
            name: name.to_string(),
            subjects: vec![bind.to_string()],
            ..Default::default()
        })
        .await
    {
        Ok(_) => {}
        Err(e) if stream_already_exists(&e) => {}
        Err(e) => panic!("get-or-create fixed fabric stream {name} binding {bind}: {e}"),
    }
}

pub async fn get_or_create_published_language(js: &jetstream::Context) {
    get_or_create_bucket(js, KV_PUBLISHED_LANGUAGE).await;
}

pub async fn get_or_create_bucket(
    js: &jetstream::Context,
    bucket: &str,
) -> async_nats::jetstream::kv::Store {
    if let Ok(store) = js.get_key_value(bucket).await {
        return store;
    }
    match js
        .create_key_value(jetstream::kv::Config {
            bucket: bucket.to_string(),
            history: 8,
            ..Default::default()
        })
        .await
    {
        Ok(store) => store,
        Err(e) if bucket_already_exists(&e) => js
            .get_key_value(bucket)
            .await
            .unwrap_or_else(|e| panic!("re-get kv bucket {bucket} after concurrent create: {e}")),
        Err(e) => panic!("get-or-create kv bucket {bucket}: {e}"),
    }
}
