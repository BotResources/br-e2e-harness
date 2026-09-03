use br_util_nats_fabric::KvKey;
use uuid::Uuid;

use crate::error::Result;
use crate::harness::PassportHarness;
use crate::vectors::{Vector, frozen_wire};

#[derive(Debug, Clone)]
pub struct SealedSeed {
    pub raw: String,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub kv_key: KvKey,
}

#[derive(Default)]
pub struct SealedSeeder;

impl SealedSeeder {
    pub fn new() -> Self {
        Self
    }

    pub async fn seed(&self, harness: &PassportHarness, vector: Vector) -> SealedSeed {
        let frozen = frozen_wire().get(vector);
        harness.pl_put_raw(&frozen.kv_key, &frozen.value).await;
        SealedSeed {
            raw: frozen.token.clone(),
            user_id: frozen.actor_id,
            token_id: frozen.token_id,
            kv_key: frozen.kv_key.clone(),
        }
    }

    pub async fn revoke(&self, harness: &PassportHarness, seed: &SealedSeed) -> Result<()> {
        harness.pl_retract(&seed.kv_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_descriptor_mirrors_its_frozen_vector() {
        let frozen = frozen_wire().get(Vector::FaithfulHuman);
        assert!(frozen.token.starts_with("brk_"));
        assert_ne!(frozen.actor_id, Uuid::nil());
        assert_ne!(frozen.token_id, Uuid::nil());
        assert!(!frozen.value.is_empty());
    }
}
