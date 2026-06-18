use async_nats::jetstream::kv::Store;
use br_core_auth::{BearerTokenEntry, bearer_token_key};
use uuid::Uuid;

pub const BEARER_BUCKET: &str = "bearer_tokens";

#[derive(thiserror::Error, Debug)]
pub enum BearerSeedError {
    #[error("serializing BearerTokenEntry: {0}")]
    Serialize(String),
    #[error("kv put '{key}': {detail}")]
    Put { key: String, detail: String },
    #[error("kv delete '{key}': {detail}")]
    Delete { key: String, detail: String },
}

#[derive(Debug, Clone)]
pub struct SeededToken {
    pub raw: String,
    pub email: String,
    pub token_id: Uuid,
}

pub struct BearerSeeder {
    pub(crate) store: Store,
}

impl BearerSeeder {
    pub async fn seed(&self, namespace: &str, label: &str) -> Result<SeededToken, BearerSeedError> {
        let raw = format!("brk_{}", Uuid::now_v7().simple());
        let email = format!("{label}+{namespace}@conformance.test");
        let token_id = Uuid::now_v7();
        let entry = BearerTokenEntry {
            email: email.clone(),
            token_id,
        };
        let value =
            serde_json::to_vec(&entry).map_err(|e| BearerSeedError::Serialize(e.to_string()))?;
        let key = bearer_token_key(&raw);
        self.store
            .put(key.clone(), value.into())
            .await
            .map_err(|e| BearerSeedError::Put {
                key: key.clone(),
                detail: e.to_string(),
            })?;
        Ok(SeededToken {
            raw,
            email,
            token_id,
        })
    }

    pub async fn revoke(&self, token: &SeededToken) -> Result<(), BearerSeedError> {
        let key = bearer_token_key(&token.raw);
        self.store
            .delete(key.clone())
            .await
            .map_err(|e| BearerSeedError::Delete {
                key,
                detail: e.to_string(),
            })
    }
}

pub fn unknown_bearer() -> String {
    format!("brk_{}", Uuid::now_v7().simple())
}
