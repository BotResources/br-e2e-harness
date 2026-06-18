use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{KvKey, KvPrefix};

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
