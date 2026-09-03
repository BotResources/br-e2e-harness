use super::catalogue::Vector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealField {
    Nonce,
    Ciphertext,
}

impl SealField {
    pub fn name(self) -> &'static str {
        match self {
            SealField::Nonce => "nonce",
            SealField::Ciphertext => "ciphertext",
        }
    }

    pub fn untouched(self) -> SealField {
        match self {
            SealField::Nonce => SealField::Ciphertext,
            SealField::Ciphertext => SealField::Nonce,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutation {
    ByteFlip(SealField),
    UnknownField,
}

pub const TWINS: [(Vector, Vector, Mutation); 3] = [
    (
        Vector::TamperedCiphertextFaithful,
        Vector::TamperedCiphertextCorrupt,
        Mutation::ByteFlip(SealField::Ciphertext),
    ),
    (
        Vector::TamperedNonceFaithful,
        Vector::TamperedNonceCorrupt,
        Mutation::ByteFlip(SealField::Nonce),
    ),
    (
        Vector::UnreadableFaithful,
        Vector::UnreadableCorrupt,
        Mutation::UnknownField,
    ),
];

pub fn wire_label(mutation: Option<Mutation>) -> &'static str {
    match mutation {
        None => "none",
        Some(Mutation::ByteFlip(field)) => field.name(),
        Some(Mutation::UnknownField) => "unreadable",
    }
}

pub fn corruption_from_wire(raw: &str) -> Result<Option<Mutation>, String> {
    match raw {
        "none" => Ok(None),
        "ciphertext" => Ok(Some(Mutation::ByteFlip(SealField::Ciphertext))),
        "nonce" => Ok(Some(Mutation::ByteFlip(SealField::Nonce))),
        "unreadable" => Ok(Some(Mutation::UnknownField)),
        other => Err(format!("unknown corruption {other:?}")),
    }
}

pub fn declared_mutation_of(name: &str) -> Option<Mutation> {
    TWINS
        .iter()
        .find(|(_, corrupt, _)| corrupt.name() == name)
        .map(|(_, _, mutation)| *mutation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wire_label_round_trips_through_the_typed_mutation() {
        for mutation in [
            None,
            Some(Mutation::ByteFlip(SealField::Nonce)),
            Some(Mutation::ByteFlip(SealField::Ciphertext)),
            Some(Mutation::UnknownField),
        ] {
            assert_eq!(
                corruption_from_wire(wire_label(mutation)).expect("a rendered label is readable"),
                mutation
            );
        }
    }

    #[test]
    fn an_unknown_corruption_label_is_rejected() {
        assert!(corruption_from_wire("shredded").is_err());
        assert!(corruption_from_wire("").is_err());
    }

    #[test]
    fn a_seal_field_names_itself_and_its_untouched_sibling() {
        for field in [SealField::Nonce, SealField::Ciphertext] {
            assert_ne!(field.name(), field.untouched().name());
            assert_eq!(field.untouched().untouched(), field);
        }
    }

    #[test]
    fn only_the_corrupt_half_of_a_pair_declares_a_mutation() {
        for (faithful, corrupt, mutation) in TWINS {
            assert_eq!(declared_mutation_of(corrupt.name()), Some(mutation));
            assert_eq!(declared_mutation_of(faithful.name()), None);
        }
        assert_eq!(declared_mutation_of("faithful-human"), None);
    }
}
