#![cfg(feature = "nats-fabric")]

use br_scope_declaration_contract::{accepted_event_coords, declare_command_coords};
use br_test_harness::fabric_nats::BareFabricNats;
use br_test_harness::{FabricTestNats, WidenedDurable};
use br_util_nats_fabric::{FabricError, INTEGRATION_EVT};

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn start_provisions_the_two_fixed_streams_and_a_filter_identical_durable() {
    let harness = FabricTestNats::start().await;
    assert!(harness.fixed_streams_present().await);

    let coords = declare_command_coords().expect("declare command coords");
    let harness = harness
        .with_command_durable(&coords, "declare_worker")
        .await;

    harness
        .fabric()
        .verify_command_durable(&coords, &harness.durable("declare_worker"))
        .await
        .expect("the harness durable filter must match what the lib binds");

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_widened_durable_makes_the_lib_bind_fail_with_filter_mismatch() {
    let coords = accepted_event_coords().expect("accepted event coords");
    let (harness, marker) = FabricTestNats::start()
        .await
        .with_widened_durable(INTEGRATION_EVT, "greedy", "integration.evt.>")
        .await;
    let WidenedDurable { durable, .. } = marker;

    let err = harness
        .fabric()
        .verify_event_durable(&coords, &durable)
        .await
        .expect_err("a widened durable must be rejected by the lib's filter check");
    assert!(matches!(err, FabricError::FilterMismatch { .. }));

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn published_language_is_get_or_create_and_is_never_wiped() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    let again = harness.with_published_language().await;
    again.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn double_provisioning_a_shared_nats_is_idempotent_and_never_wipes() {
    let owner = FabricTestNats::start()
        .await
        .with_published_language()
        .await
        .with_bearer_tokens()
        .await;
    let url = owner.url();

    let seeder = owner.bearer_seeder();
    let seeded = seeder
        .seed("double-provision", "race")
        .await
        .expect("seed a bearer token before the second provision pass");

    let second = FabricTestNats::connect(&url)
        .await
        .with_published_language()
        .await
        .with_bearer_tokens()
        .await;

    assert!(second.fixed_streams_present().await);
    assert!(second.published_language_present().await);

    let survived = second
        .bearer_seeder()
        .seed("double-provision-2", "race2")
        .await
        .expect("seed against the re-provisioned bearer bucket");
    assert_ne!(survived.raw, seeded.raw);

    second.shutdown().await;
    owner.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_missing_fixed_stream_makes_the_lib_bind_fail_loud() {
    let bare = BareFabricNats::with_only_event_stream().await;
    let coords = declare_command_coords().expect("declare command coords");

    let err = bare.assert_missing_command_stream(&coords, "absent").await;
    assert!(matches!(err, FabricError::Consume { .. }));
    assert!(bare.command_stream_absent().await);

    bare.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn the_published_language_bucket_is_absent_until_opted_in() {
    let bare = BareFabricNats::without_fixed_streams().await;
    assert!(bare.published_language_absent().await);
    bare.shutdown().await;
}
