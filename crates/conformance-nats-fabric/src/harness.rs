use async_nats::jetstream::kv::Store;
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{KV_PUBLISHED_LANGUAGE, KvKey, KvPrefix};

use crate::error::{ConformanceError, Result};

pub async fn published_language_store(harness: &FabricTestNats) -> Result<Store> {
    harness
        .jetstream()
        .get_key_value(KV_PUBLISHED_LANGUAGE)
        .await
        .map_err(|e| ConformanceError::Anchor(format!("published-language bucket absent: {e}")))
}

pub fn namespaced_key(harness: &FabricTestNats, suffix: &str) -> KvKey {
    let prefix = harness.key_prefix();
    KvKey::new(format!("{}{suffix}", prefix.as_str()))
        .expect("a run-namespaced published-language key is valid")
}

pub fn namespaced_prefix(harness: &FabricTestNats, suffix: &str) -> KvPrefix {
    let prefix = harness.key_prefix();
    KvPrefix::new(format!("{}{suffix}", prefix.as_str()))
        .expect("a run-namespaced published-language prefix is valid")
}

pub async fn put_raw(store: &Store, key: &KvKey, bytes: &[u8]) -> Result<()> {
    store
        .put(key.as_str(), bytes.to_vec().into())
        .await
        .map_err(|e| ConformanceError::Anchor(format!("raw kv put on {}: {e}", key.as_str())))?;
    Ok(())
}
