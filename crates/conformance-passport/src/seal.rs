use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use br_auth_contract::{BEARER_SEAL_KEY_LEN, BearerEntry, BearerSealKey};
use br_auth_identity_util::BearerPublisher;
use br_core_kernel::{Actor, UserId};
use br_util_nats_fabric::Fabric;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub const SEAL_KEY: [u8; BEARER_SEAL_KEY_LEN] = [
    0x1f, 0x2e, 0x3d, 0x4c, 0x5b, 0x6a, 0x79, 0x88, 0x97, 0xa6, 0xb5, 0xc4, 0xd3, 0xe2, 0xf1, 0x00,
    0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a, 0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2, 0xe1, 0xf0,
];

pub const WRONG_SEAL_KEY: [u8; BEARER_SEAL_KEY_LEN] = [0x99; BEARER_SEAL_KEY_LEN];

pub fn seal_key() -> BearerSealKey {
    BearerSealKey::from_bytes(&SEAL_KEY).expect("SEAL_KEY is exactly 32 bytes")
}

pub fn wrong_seal_key() -> BearerSealKey {
    BearerSealKey::from_bytes(&WRONG_SEAL_KEY).expect("WRONG_SEAL_KEY is exactly 32 bytes")
}

pub fn seal_key_b64() -> String {
    STANDARD.encode(SEAL_KEY)
}

#[derive(Debug, Clone)]
pub struct SealedSeed {
    pub raw: String,
    pub user_id: Uuid,
    pub token_id: Uuid,
}

pub struct SealedSeeder {
    publisher: BearerPublisher,
}

impl SealedSeeder {
    pub async fn open(fabric: &Fabric, key: BearerSealKey) -> Result<Self> {
        let publisher = BearerPublisher::open(fabric, key)
            .await
            .map_err(|e| ConformanceError::Seed(e.to_string()))?;
        Ok(Self { publisher })
    }

    pub async fn seed(&self, namespace: &str, label: &str) -> Result<SealedSeed> {
        let raw = format!("brk_{label}_{namespace}_{}", Uuid::now_v7().simple());
        let user_id = Uuid::now_v7();
        let token_id = Uuid::now_v7();
        let entry = BearerEntry {
            actor: Actor::Human(UserId::from(user_id)),
            token_id,
        };
        self.publisher
            .put_bearer(&raw, &entry)
            .await
            .map_err(|e| ConformanceError::Seed(e.to_string()))?;
        Ok(SealedSeed {
            raw,
            user_id,
            token_id,
        })
    }

    pub async fn revoke(&self, seed: &SealedSeed) -> Result<()> {
        self.publisher
            .delete_bearer(&seed.raw)
            .await
            .map_err(|e| ConformanceError::Seed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_key_is_thirty_two_bytes_and_base64_round_trips() {
        assert_eq!(SEAL_KEY.len(), BEARER_SEAL_KEY_LEN);
        let decoded = STANDARD.decode(seal_key_b64()).expect("base64-std decodes");
        assert_eq!(decoded, SEAL_KEY.to_vec());
    }

    #[test]
    fn the_wrong_key_differs_from_the_correct_one() {
        assert_ne!(SEAL_KEY, WRONG_SEAL_KEY);
    }
}
