use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value};

use super::catalogue::Vector;
use super::frozen::WireVector;
use super::mutation::{Mutation, SealField, TWINS, declared_mutation_of, wire_label};

pub(super) fn assert_every_corrupt_vector_is_a_controlled_twin(
    vectors: &[WireVector],
) -> Result<(), String> {
    assert_declared_corruptions_match_the_twin_table(vectors)?;
    for (faithful, corrupt, mutation) in TWINS {
        let faithful = named(vectors, faithful)?;
        let corrupt = named(vectors, corrupt)?;
        assert_one_identity(faithful, corrupt)?;
        let before = envelope(faithful)?;
        let after = envelope(corrupt)?;
        let outcome = match mutation {
            Mutation::UnknownField => assert_only_an_unknown_key_was_added(&before, &after),
            Mutation::ByteFlip(field) => assert_byte_zero_flipped(&before, &after, field),
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

fn assert_declared_corruptions_match_the_twin_table(vectors: &[WireVector]) -> Result<(), String> {
    for vector in vectors {
        let declared = declared_mutation_of(&vector.name);
        if vector.corruption == declared {
            continue;
        }
        return Err(match declared {
            None => format!(
                "vector {} declares corruption {:?} but no twin pair names it as a corrupt half",
                vector.name,
                wire_label(vector.corruption)
            ),
            Some(expected) => format!(
                "vector {} declares corruption {:?} where its twin pair names {:?}",
                vector.name,
                wire_label(vector.corruption),
                wire_label(Some(expected))
            ),
        });
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

fn assert_byte_zero_flipped(
    before: &Map<String, Value>,
    after: &Map<String, Value>,
    field: SealField,
) -> Result<(), String> {
    if before.len() != 2 || after.len() != 2 {
        return Err("a tampered twin carries exactly nonce and ciphertext".to_string());
    }
    let untouched = field.untouched();
    if field_of(before, untouched)? != field_of(after, untouched)? {
        return Err(format!("the {} changed too", untouched.name()));
    }
    let from = decode(&field_of(before, field)?, field)?;
    let to = decode(&field_of(after, field)?, field)?;
    if from.len() != to.len() {
        return Err(format!("the {} changed length", field.name()));
    }
    let differing: Vec<usize> = from
        .iter()
        .zip(to.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, _)| index)
        .collect();
    if differing != [0] {
        return Err(format!(
            "bytes {differing:?} of the {} differ, the declared mutation flips byte 0 alone",
            field.name()
        ));
    }
    if to[0] != from[0] ^ 0xff {
        return Err(format!(
            "byte 0 of the {} is not its faithful byte xor 0xff",
            field.name()
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

fn field_of(object: &Map<String, Value>, field: SealField) -> Result<String, String> {
    object
        .get(field.name())
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("the envelope has no string field {:?}", field.name()))
}

fn decode(b64: &str, field: SealField) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(b64)
        .map_err(|e| format!("the {} is not base64-std: {e}", field.name()))
}

#[cfg(test)]
#[path = "twins_tests.rs"]
mod tests;
