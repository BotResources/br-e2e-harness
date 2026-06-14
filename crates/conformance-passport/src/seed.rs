use async_nats::jetstream::kv;
use br_core_auth::{BearerTokenEntry, bearer_token_key};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub const BEARER_BUCKET: &str = "bearer_tokens";

#[derive(Debug, Clone)]
pub struct SeededToken {
    pub raw: String,
    pub email: String,
    pub token_id: Uuid,
}

pub struct BearerSeeder {
    store: kv::Store,
}

impl BearerSeeder {
    pub fn new(store: kv::Store) -> Self {
        Self { store }
    }

    pub async fn seed(&self, namespace: &str, label: &str) -> Result<SeededToken> {
        let raw = format!("brk_{}", Uuid::now_v7().simple());
        let email = format!("{label}+{namespace}@conformance.test");
        let token_id = Uuid::now_v7();
        let entry = BearerTokenEntry {
            email: email.clone(),
            token_id,
        };
        let value = serde_json::to_vec(&entry)
            .map_err(|e| ConformanceError::Seed(format!("serializing BearerTokenEntry: {e}")))?;
        self.store
            .put(bearer_token_key(&raw), value.into())
            .await
            .map_err(|e| {
                ConformanceError::Seed(format!("put '{}': {e}", bearer_token_key(&raw)))
            })?;
        Ok(SeededToken {
            raw,
            email,
            token_id,
        })
    }

    pub async fn revoke(&self, token: &SeededToken) -> Result<()> {
        self.store
            .delete(bearer_token_key(&token.raw))
            .await
            .map_err(|e| ConformanceError::Seed(format!("delete '{}': {e}", token.raw)))
    }
}

pub fn unknown_bearer() -> String {
    format!("brk_{}", Uuid::now_v7().simple())
}
