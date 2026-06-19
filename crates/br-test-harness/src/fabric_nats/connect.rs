use std::time::Duration;

use async_nats::jetstream::context::{
    CreateKeyValueError, CreateStreamError, CreateStreamErrorKind,
};
use async_nats::jetstream::{self, ErrorCode, stream};
use br_util_nats_fabric::{KV_EPHEMERAL_AUTH, KV_PUBLISHED_LANGUAGE};

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

const EPHEMERAL_AUTH_MAX_AGE: Duration = Duration::from_secs(3600);
const EPHEMERAL_AUTH_MARKER_TTL: Duration = Duration::from_secs(1);

pub async fn get_or_create_ephemeral_auth(
    js: &jetstream::Context,
) -> async_nats::jetstream::kv::Store {
    get_or_create_bucket_with_config(
        js,
        jetstream::kv::Config {
            bucket: KV_EPHEMERAL_AUTH.to_string(),
            history: 8,
            max_age: EPHEMERAL_AUTH_MAX_AGE,
            limit_markers: Some(EPHEMERAL_AUTH_MARKER_TTL),
            ..Default::default()
        },
    )
    .await
}

pub async fn get_or_create_bucket(
    js: &jetstream::Context,
    bucket: &str,
) -> async_nats::jetstream::kv::Store {
    get_or_create_bucket_with_config(
        js,
        jetstream::kv::Config {
            bucket: bucket.to_string(),
            history: 8,
            ..Default::default()
        },
    )
    .await
}

async fn get_or_create_bucket_with_config(
    js: &jetstream::Context,
    config: jetstream::kv::Config,
) -> async_nats::jetstream::kv::Store {
    let bucket = config.bucket.clone();
    if let Ok(store) = js.get_key_value(&bucket).await {
        return store;
    }
    match js.create_key_value(config).await {
        Ok(store) => store,
        Err(e) if bucket_already_exists(&e) => js
            .get_key_value(&bucket)
            .await
            .unwrap_or_else(|e| panic!("re-get kv bucket {bucket} after concurrent create: {e}")),
        Err(e) => panic!("get-or-create kv bucket {bucket}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EPHEMERAL_AUTH_MARKER_TTL, EPHEMERAL_AUTH_MAX_AGE, bucket_already_exists,
        get_or_create_ephemeral_auth, stream_already_exists,
    };
    use crate::spawned_nats::SpawnedNats;
    use async_nats::jetstream::{self, stream};
    use br_util_nats_fabric::KV_EPHEMERAL_AUTH;

    async fn js_context(nats: &SpawnedNats) -> jetstream::Context {
        let client = async_nats::connect(nats.url())
            .await
            .expect("connect to the spawned nats-server");
        jetstream::new(client)
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn stream_already_exists_detects_the_real_jetstream_error() {
        let nats = SpawnedNats::start().await;
        let js = js_context(&nats).await;

        let name = "ABSORB_STREAM".to_string();
        js.create_stream(stream::Config {
            name: name.clone(),
            subjects: vec!["absorb.one".to_string()],
            ..Default::default()
        })
        .await
        .expect("first create of the stream succeeds");

        let err = js
            .create_stream(stream::Config {
                name,
                subjects: vec!["absorb.two".to_string()],
                ..Default::default()
            })
            .await
            .expect_err(
                "re-creating the same stream name with a DIFFERENT config returns \
                 the real STREAM_NAME_EXIST error (identical-config re-create returns Ok)",
            );

        assert!(
            stream_already_exists(&err),
            "the predicate must catch the real STREAM_NAME_EXIST error: {err:?}"
        );

        nats.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn bucket_already_exists_detects_the_real_kv_error() {
        let nats = SpawnedNats::start().await;
        let js = js_context(&nats).await;

        let bucket = "absorb-bucket".to_string();
        js.create_key_value(jetstream::kv::Config {
            bucket: bucket.clone(),
            history: 8,
            ..Default::default()
        })
        .await
        .expect("first create of the kv bucket succeeds");

        let err = js
            .create_key_value(jetstream::kv::Config {
                bucket,
                history: 1,
                ..Default::default()
            })
            .await
            .expect_err("re-creating the same kv bucket returns the real already-exists error");

        assert!(
            bucket_already_exists(&err),
            "the source()-chain downcast must catch the real kv already-exists error \
             async-nats 0.48 produces: {err:?}"
        );

        nats.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn a_non_existence_kv_create_error_is_not_absorbed() {
        let nats = SpawnedNats::start().await;
        let js = js_context(&nats).await;

        let err = js
            .create_key_value(jetstream::kv::Config {
                bucket: "absorb bucket with spaces".to_string(),
                history: 8,
                ..Default::default()
            })
            .await
            .expect_err("an invalid bucket name is rejected, not an already-exists error");

        assert!(
            !bucket_already_exists(&err),
            "a genuine (non already-exists) create failure must NOT be absorbed, \
             so the panic path is preserved: {err:?}"
        );

        nats.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn ephemeral_auth_bucket_is_created_with_the_ttl_marker_config() {
        let nats = SpawnedNats::start().await;
        let js = js_context(&nats).await;

        get_or_create_ephemeral_auth(&js).await;
        get_or_create_ephemeral_auth(&js).await;

        let stream = js
            .get_stream(format!("KV_{KV_EPHEMERAL_AUTH}"))
            .await
            .expect("the EPHEMERAL_AUTH kv stream exists after provisioning");
        let info = stream.cached_info();
        assert_eq!(info.config.max_age, EPHEMERAL_AUTH_MAX_AGE);
        assert!(
            info.config.allow_message_ttl,
            "limit_markers must enable per-message ttl on the kv stream"
        );
        assert_eq!(
            info.config.subject_delete_marker_ttl,
            Some(EPHEMERAL_AUTH_MARKER_TTL),
            "the delete-marker ttl floor must match the chosen marker ttl"
        );

        nats.shutdown().await;
    }
}
