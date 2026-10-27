use br_test_harness::{BareFabricNats, FabricTestNats};
use conformance_nats_fabric::checks::integration::{
    DEAD_COMMAND_SUBJECT, assert_dead_grammar_fails_loud, assert_no_fixed_stream_captured,
};

async fn attached_without_fixed_streams() -> (BareFabricNats, FabricTestNats) {
    let bare = BareFabricNats::without_fixed_streams().await;
    let harness = FabricTestNats::attach_without_provisioning(&bare.url()).await;
    (bare, harness)
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
#[should_panic(expected = "must exist")]
async fn the_capture_check_refuses_a_server_without_fixed_streams() {
    let (_bare, harness) = attached_without_fixed_streams().await;
    let _ = assert_no_fixed_stream_captured(&harness, DEAD_COMMAND_SUBJECT).await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
#[should_panic(expected = "must exist")]
async fn the_dead_grammar_check_refuses_a_server_without_fixed_streams() {
    let (_bare, harness) = attached_without_fixed_streams().await;
    assert_dead_grammar_fails_loud(&harness, DEAD_COMMAND_SUBJECT).await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
#[should_panic(expected = "the broker did not answer")]
async fn the_capture_check_refuses_an_unreadable_broker() {
    let (bare, harness) = attached_without_fixed_streams().await;
    bare.shutdown().await;
    let _ = assert_no_fixed_stream_captured(&harness, DEAD_COMMAND_SUBJECT).await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn both_checks_pass_on_a_server_with_the_fixed_streams() {
    let harness = FabricTestNats::start().await;
    assert_dead_grammar_fails_loud(&harness, DEAD_COMMAND_SUBJECT).await;
    assert_no_fixed_stream_captured(&harness, DEAD_COMMAND_SUBJECT)
        .await
        .expect("no fixed stream captured the dead grammar");
    harness.shutdown().await;
}
