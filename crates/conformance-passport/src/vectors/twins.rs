use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value};

use super::catalogue::Vector;
use super::frozen::WireVector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutation {
    CiphertextByteFlip,
    NonceByteFlip,
    UnknownField,
}

impl Mutation {
    fn declared_field(self) -> &'static str {
        match self {
            Mutation::CiphertextByteFlip => "ciphertext",
            Mutation::NonceByteFlip => "nonce",
            Mutation::UnknownField => "",
        }
    }
}

pub const TWINS: [(Vector, Vector, Mutation); 3] = [
    (
        Vector::TamperedCiphertextFaithful,
        Vector::TamperedCiphertextCorrupt,
        Mutation::CiphertextByteFlip,
    ),
    (
        Vector::TamperedNonceFaithful,
        Vector::TamperedNonceCorrupt,
        Mutation::NonceByteFlip,
    ),
    (
        Vector::UnreadableFaithful,
        Vector::UnreadableCorrupt,
        Mutation::UnknownField,
    ),
];

pub(super) fn assert_every_twin_is_its_faithful_plus_the_declared_mutation(
    vectors: &[WireVector],
) -> Result<(), String> {
    for (faithful, corrupt, mutation) in TWINS {
        let faithful = named(vectors, faithful)?;
        let corrupt = named(vectors, corrupt)?;
        assert_one_identity(faithful, corrupt)?;
        let before = envelope(faithful)?;
        let after = envelope(corrupt)?;
        let outcome = match mutation {
            Mutation::UnknownField => assert_only_an_unknown_key_was_added(&before, &after),
            flip => assert_one_byte_flipped(&before, &after, flip.declared_field()),
        };
        outcome.map_err(|detail| {
            format!(
                "vector {} must be {} with the declared mutation applied: {detail}",
                corrupt.name, faithful.name
            )
        })?;
    }
    Ok(())
}

fn named(vectors: &[WireVector], vector: Vector) -> Result<&WireVector, String> {
    vectors
        .iter()
        .find(|candidate| candidate.name == vector.name())
        .ok_or_else(|| format!("the vector file has no entry named {:?}", vector.name()))
}

fn assert_one_identity(faithful: &WireVector, corrupt: &WireVector) -> Result<(), String> {
    let same = faithful.token == corrupt.token
        && faithful.kv_key == corrupt.kv_key
        && faithful.actor_id == corrupt.actor_id
        && faithful.token_id == corrupt.token_id;
    if !same {
        return Err(format!(
            "vectors {} and {} are not a twin pair: they differ beyond the declared mutation",
            faithful.name, corrupt.name
        ));
    }
    if faithful.value == corrupt.value {
        return Err(format!(
            "vector {} carries the untouched bytes of {}",
            corrupt.name, faithful.name
        ));
    }
    Ok(())
}

fn envelope(vector: &WireVector) -> Result<Map<String, Value>, String> {
    match serde_json::from_slice(&vector.value) {
        Ok(Value::Object(object)) => Ok(object),
        Ok(_) => Err(format!(
            "vector {}: the value is not an object",
            vector.name
        )),
        Err(e) => Err(format!(
            "vector {}: the value is not JSON: {e}",
            vector.name
        )),
    }
}

fn assert_one_byte_flipped(
    before: &Map<String, Value>,
    after: &Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    let untouched = if field == "nonce" {
        "ciphertext"
    } else {
        "nonce"
    };
    if before.len() != 2 || after.len() != 2 {
        return Err("a tampered twin carries exactly nonce and ciphertext".to_string());
    }
    if field_of(before, untouched)? != field_of(after, untouched)? {
        return Err(format!("the {untouched} changed too"));
    }
    let from = decode(&field_of(before, field)?, field)?;
    let to = decode(&field_of(after, field)?, field)?;
    if from.len() != to.len() {
        return Err(format!("the {field} changed length"));
    }
    let differing = from.iter().zip(to.iter()).filter(|(a, b)| a != b).count();
    if differing != 1 {
        return Err(format!(
            "{differing} bytes of the {field} differ, the declared mutation flips exactly one"
        ));
    }
    Ok(())
}

fn assert_only_an_unknown_key_was_added(
    before: &Map<String, Value>,
    after: &Map<String, Value>,
) -> Result<(), String> {
    let mut stripped = after.clone();
    let extra: Vec<String> = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .cloned()
        .collect();
    if extra.len() != 1 {
        return Err(format!(
            "exactly one unknown key must be added, found {extra:?}"
        ));
    }
    stripped.remove(&extra[0]);
    if &stripped != before {
        return Err(format!(
            "the envelope must be the faithful one plus {:?} and nothing else",
            extra[0]
        ));
    }
    Ok(())
}

fn field_of(object: &Map<String, Value>, field: &str) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("the envelope has no string field {field:?}"))
}

fn decode(b64: &str, field: &str) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(b64)
        .map_err(|e| format!("the {field} is not base64-std: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::frozen::{FROZEN, parse};

    fn doctored(name: &str, mutate: impl Fn(&mut Value)) -> String {
        let mut root: Value = serde_json::from_str(FROZEN).expect("the frozen file is JSON");
        let entry = root["vectors"]
            .as_array_mut()
            .expect("vectors is an array")
            .iter_mut()
            .find(|entry| entry["name"] == Value::String(name.to_string()))
            .expect("the named vector exists");
        mutate(entry);
        serde_json::to_string(&root).expect("the doctored file serialises")
    }

    fn envelope_of(entry: &Value) -> Map<String, Value> {
        let raw = STANDARD
            .decode(entry["value_b64"].as_str().expect("value_b64 is a string"))
            .expect("value_b64 is base64-std");
        serde_json::from_slice(&raw).expect("the value is a JSON object")
    }

    fn store(entry: &mut Value, envelope: &Map<String, Value>) {
        let body = serde_json::to_vec(envelope).expect("the envelope serialises");
        entry["value_b64"] = Value::String(STANDARD.encode(body));
    }

    fn flip(envelope: &mut Map<String, Value>, field: &str, index: usize) {
        let mut raw = STANDARD
            .decode(envelope[field].as_str().expect("the field is a string"))
            .expect("the field is base64-std");
        raw[index] ^= 0xff;
        envelope[field] = Value::String(STANDARD.encode(raw));
    }

    #[test]
    fn the_embedded_file_carries_only_controlled_twins() {
        parse(FROZEN).expect("the committed vector file must pass the twin check");
    }

    #[test]
    fn a_corrupt_vector_sealed_under_another_nonce_is_rejected() {
        let foreign = doctored("tampered-ciphertext-corrupt", |entry| {
            let mut envelope = envelope_of(entry);
            flip(&mut envelope, "nonce", 0);
            store(entry, &envelope);
        });
        let err = parse(&foreign).expect_err("a twin sealed under another nonce must be rejected");
        assert!(err.contains("tampered-ciphertext-corrupt"), "{err}");
        assert!(err.contains("nonce changed too"), "{err}");
    }

    #[test]
    fn a_corrupt_vector_differing_by_more_than_the_declared_flip_is_rejected() {
        let widened = doctored("tampered-nonce-corrupt", |entry| {
            let mut envelope = envelope_of(entry);
            flip(&mut envelope, "nonce", 1);
            store(entry, &envelope);
        });
        let err = parse(&widened).expect_err("a two-byte difference must be rejected");
        assert!(err.contains("flips exactly one"), "{err}");
    }

    #[test]
    fn an_unreadable_twin_that_also_touched_the_seal_is_rejected() {
        let widened = doctored("unreadable-corrupt", |entry| {
            let mut envelope = envelope_of(entry);
            flip(&mut envelope, "ciphertext", 0);
            store(entry, &envelope);
        });
        let err = parse(&widened).expect_err("an unreadable twin must not re-seal");
        assert!(err.contains("and nothing else"), "{err}");
    }

    #[test]
    fn an_unreadable_twin_carrying_no_unknown_key_is_rejected() {
        let stripped = doctored("unreadable-corrupt", |entry| {
            let mut envelope = envelope_of(entry);
            envelope.remove("evil");
            store(entry, &envelope);
        });
        let err = parse(&stripped).expect_err("an unreadable twin must add its unknown key");
        assert!(err.contains("exactly one unknown key"), "{err}");
    }

    #[test]
    fn a_twin_pair_that_stopped_sharing_its_identity_is_rejected() {
        let split = doctored("tampered-ciphertext-corrupt", |entry| {
            entry["token_id"] = Value::String("0190c0de-9999-7e5f-8a9b-0c1d2e3f4a5b".to_string());
        });
        let err = parse(&split).expect_err("a twin carrying another identity must be rejected");
        assert!(err.contains("not a twin pair"), "{err}");
    }

    #[test]
    fn a_missing_twin_half_is_rejected() {
        let mut root: Value = serde_json::from_str(FROZEN).expect("the frozen file is JSON");
        root["vectors"]
            .as_array_mut()
            .expect("vectors is an array")
            .retain(|entry| entry["name"] != Value::String("unreadable-corrupt".to_string()));
        let err = parse(&serde_json::to_string(&root).expect("serialises"))
            .expect_err("a half-declared pair must be rejected");
        assert!(err.contains("unreadable-corrupt"), "{err}");
    }
}
