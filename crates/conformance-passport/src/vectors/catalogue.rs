#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vector {
    FaithfulHuman,
    FaithfulHumanSecond,
    FaithfulService,
    Revoked,
    KvError,
    WrongKey,
    TamperedCiphertextFaithful,
    TamperedCiphertextCorrupt,
    TamperedNonceFaithful,
    TamperedNonceCorrupt,
    UnreadableFaithful,
    UnreadableCorrupt,
}

pub const EVERY_VECTOR: [Vector; 12] = [
    Vector::FaithfulHuman,
    Vector::FaithfulHumanSecond,
    Vector::FaithfulService,
    Vector::Revoked,
    Vector::KvError,
    Vector::WrongKey,
    Vector::TamperedCiphertextFaithful,
    Vector::TamperedCiphertextCorrupt,
    Vector::TamperedNonceFaithful,
    Vector::TamperedNonceCorrupt,
    Vector::UnreadableFaithful,
    Vector::UnreadableCorrupt,
];

impl Vector {
    pub fn name(self) -> &'static str {
        match self {
            Vector::FaithfulHuman => "faithful-human",
            Vector::FaithfulHumanSecond => "faithful-human-second",
            Vector::FaithfulService => "faithful-service",
            Vector::Revoked => "revoked",
            Vector::KvError => "kv-error",
            Vector::WrongKey => "wrong-key",
            Vector::TamperedCiphertextFaithful => "tampered-ciphertext-faithful",
            Vector::TamperedCiphertextCorrupt => "tampered-ciphertext-corrupt",
            Vector::TamperedNonceFaithful => "tampered-nonce-faithful",
            Vector::TamperedNonceCorrupt => "tampered-nonce-corrupt",
            Vector::UnreadableFaithful => "unreadable-faithful",
            Vector::UnreadableCorrupt => "unreadable-corrupt",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_is_listed_once_under_a_distinct_name() {
        let mut names: Vec<&str> = EVERY_VECTOR.iter().map(|v| v.name()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "two variants share a vector name");
    }

    #[test]
    fn a_vector_name_is_never_empty() {
        for vector in EVERY_VECTOR {
            assert!(!vector.name().is_empty(), "{vector:?} has no name");
        }
    }
}
