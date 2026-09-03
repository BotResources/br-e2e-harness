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

fn xor_byte(envelope: &mut Map<String, Value>, field: &str, index: usize, mask: u8) {
    let mut raw = STANDARD
        .decode(envelope[field].as_str().expect("the field is a string"))
        .expect("the field is base64-std");
    raw[index] ^= mask;
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
        xor_byte(&mut envelope, "nonce", 0, 0xff);
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
        xor_byte(&mut envelope, "nonce", 1, 0xff);
        store(entry, &envelope);
    });
    let err = parse(&widened).expect_err("a two-byte difference must be rejected");
    assert!(err.contains("flips byte 0 alone"), "{err}");
}

#[test]
fn a_flip_that_is_not_byte_zero_is_rejected() {
    let elsewhere = doctored("tampered-nonce-corrupt", |entry| {
        let mut envelope = envelope_of(entry);
        xor_byte(&mut envelope, "nonce", 0, 0xff);
        xor_byte(&mut envelope, "nonce", 3, 0xff);
        store(entry, &envelope);
    });
    let err = parse(&elsewhere).expect_err("a flip away from byte 0 must be rejected");
    assert!(err.contains("bytes [3]"), "{err}");
}

#[test]
fn a_byte_zero_difference_that_is_not_a_full_flip_is_rejected() {
    let partial = doctored("tampered-ciphertext-corrupt", |entry| {
        let mut envelope = envelope_of(entry);
        xor_byte(&mut envelope, "ciphertext", 0, 0xfe);
        store(entry, &envelope);
    });
    let err = parse(&partial).expect_err("byte 0 must be the faithful byte xor 0xff");
    assert!(err.contains("xor 0xff"), "{err}");
}

#[test]
fn an_unreadable_twin_that_also_touched_the_seal_is_rejected() {
    let widened = doctored("unreadable-corrupt", |entry| {
        let mut envelope = envelope_of(entry);
        xor_byte(&mut envelope, "ciphertext", 0, 0xff);
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

#[test]
fn a_corrupt_vector_outside_the_twin_table_is_rejected() {
    let mut root: Value = serde_json::from_str(FROZEN).expect("the frozen file is JSON");
    let entries = root["vectors"].as_array_mut().expect("vectors is an array");
    let mut intruder = entries
        .iter()
        .find(|entry| entry["name"] == Value::String("faithful-human".to_string()))
        .expect("the faithful human exists")
        .clone();
    intruder["name"] = Value::String("smuggled-corrupt".to_string());
    intruder["corruption"] = Value::String("ciphertext".to_string());
    entries.push(intruder);
    let err = parse(&serde_json::to_string(&root).expect("serialises"))
        .expect_err("an untwinned corrupt vector must be rejected");
    assert!(err.contains("smuggled-corrupt"), "{err}");
    assert!(err.contains("no twin pair names it"), "{err}");
}

#[test]
fn a_corrupt_half_that_declares_no_corruption_is_rejected() {
    let hidden = doctored("tampered-nonce-corrupt", |entry| {
        entry["corruption"] = Value::String("none".to_string());
    });
    let err = parse(&hidden).expect_err("a corrupt half must declare its mutation");
    assert!(err.contains("its twin pair names \"nonce\""), "{err}");
}

#[test]
fn a_corrupt_half_that_declares_another_mutation_is_rejected() {
    let swapped = doctored("unreadable-corrupt", |entry| {
        entry["corruption"] = Value::String("ciphertext".to_string());
    });
    let err = parse(&swapped).expect_err("a corrupt half must declare its own mutation");
    assert!(err.contains("its twin pair names \"unreadable\""), "{err}");
}

#[test]
fn an_unknown_corruption_label_in_the_file_is_rejected() {
    let alien = doctored("faithful-human", |entry| {
        entry["corruption"] = Value::String("shredded".to_string());
    });
    let err = parse(&alien).expect_err("an unknown corruption label must be rejected");
    assert!(err.contains("unknown corruption"), "{err}");
}
