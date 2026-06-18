use br_test_harness::{BareFabricNats, FabricTestNats};
use conformance_nats_fabric::anchor::frozen_wire;
use conformance_nats_fabric::checks::integration::{
    assert_dead_grammar_fails_loud, assert_missing_stream_fails_loud,
    assert_no_fixed_stream_captured, assert_widened_durable_rejected, rust_command_subject,
    rust_event_subject, widen,
};
use conformance_nats_fabric::checks::projection::{
    assert_bootstrap_then_watch_is_parallel_safe, assert_prefix_watch_delivery_gap,
};
use conformance_nats_fabric::checks::published_language::{
    assert_decode_fails_closed_naming_the_key, assert_poison_from_anchor_names_the_key,
    assert_reconcile_drift_converges, assert_retract_orphan_deletes,
    parse_published_user_through_lib,
};

const DEAD_COMMAND_SUBJECT: &str = "identity.cmd.service_scope.declare.v1";

#[tokio::test]
#[ignore = "requires go toolchain; renders the frozen wire from the anchor"]
async fn anchor_subjects_match_the_lib_renderers_byte_for_byte() {
    let wire = frozen_wire().await.expect("anchor renders the frozen wire");

    assert!(!wire.command_subjects.is_empty());
    for go in &wire.command_subjects {
        assert_eq!(
            rust_command_subject(go).expect("lib renders command coords"),
            go.subject,
            "command subject drift for {go:?}"
        );
    }

    assert!(!wire.event_subjects.is_empty());
    for go in &wire.event_subjects {
        assert_eq!(
            rust_event_subject(go).expect("lib renders event coords"),
            go.subject,
            "event subject drift for {go:?}"
        );
    }
}

#[tokio::test]
#[ignore = "requires go toolchain; deserializes the anchor users through the lib"]
async fn anchor_published_users_deserialize_through_the_lib() {
    let wire = frozen_wire().await.expect("anchor renders the frozen wire");
    assert!(!wire.published_users.is_empty());
    for entry in &wire.published_users {
        let user = parse_published_user_through_lib(&entry.value)
            .unwrap_or_else(|e| panic!("anchor user {} is not a PublishedUser: {e}", entry.key));
        assert!(
            !user.email.is_empty(),
            "the published user carries an email"
        );
    }
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn a_widened_durable_is_rejected() {
    let harness = FabricTestNats::start().await;
    let (harness, marker) = widen(harness, "widened_evt").await;
    assert_widened_durable_rejected(&harness, &marker).await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn a_missing_fixed_stream_fails_loud() {
    let bare = BareFabricNats::without_fixed_streams().await;
    assert_missing_stream_fails_loud(&bare).await;
    bare.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn the_dead_grammar_fails_loud() {
    let harness = FabricTestNats::start().await;
    assert_dead_grammar_fails_loud(&harness, DEAD_COMMAND_SUBJECT).await;
    assert_no_fixed_stream_captured(&harness, DEAD_COMMAND_SUBJECT)
        .await
        .expect("no fixed stream captured the dead grammar");
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn published_language_retract_orphan_deletes() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_retract_orphan_deletes(&harness)
        .await
        .expect("retract orphan-deletes");
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn published_language_reconcile_converges() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_reconcile_drift_converges(&harness)
        .await
        .expect("reconcile converges");
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn published_language_bootstrap_then_watch_is_parallel_safe() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_bootstrap_then_watch_is_parallel_safe(&harness)
        .await
        .expect("bootstrap then watch is parallel-safe");
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn prefix_watch_delivers_slash_keyed_directory_puts() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    let delivered = assert_prefix_watch_delivery_gap(&harness)
        .await
        .expect("the gap probe runs");
    assert!(
        delivered,
        "prefix-watch must deliver a live slash-keyed put within the deadline; \
         br-util-nats-fabric v1.0.1 fixes the watch subject so the wildcard fires on real infra"
    );
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn published_language_decode_fails_closed_naming_the_key() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_decode_fails_closed_naming_the_key(&harness)
        .await
        .expect("decode fails closed naming the key");
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server + go toolchain"]
async fn published_language_anchor_poison_fails_closed_naming_the_key() {
    let wire = frozen_wire().await.expect("anchor renders the frozen wire");
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_poison_from_anchor_names_the_key(
        &harness,
        &wire.poison_user_key,
        &wire.poison_user_value,
    )
    .await
    .expect("anchor poison fails closed naming the key");
    harness.shutdown().await;
}
