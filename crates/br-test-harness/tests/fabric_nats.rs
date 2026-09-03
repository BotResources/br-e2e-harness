#![cfg(feature = "nats-fabric")]

use br_scope_declaration_contract::{accepted_event_coords, declare_command_coords};
use br_test_harness::fabric_nats::BareFabricNats;
use br_test_harness::{FabricTestNats, WidenedDurable};
use br_util_nats_fabric::{
    ConsumeErrorKind, FabricError, INTEGRATION_CMD, INTEGRATION_EVT, command_subject, event_subject,
};

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn start_provisions_the_two_fixed_streams_and_a_filter_identical_durable() {
    let harness = FabricTestNats::start().await;
    assert!(harness.fixed_streams_present().await);

    let coords = declare_command_coords().expect("declare command coords");
    let harness = harness
        .with_command_durable(&coords, "declare_worker")
        .await;

    let durable = harness.durable("declare_worker");
    harness
        .fabric()
        .verify_command_durable(&coords, &durable)
        .await
        .expect("the fixed command stream must cover the declared coordinate");
    assert_eq!(
        harness
            .durable_filter_subjects(INTEGRATION_CMD, &durable)
            .await,
        vec![command_subject(&coords)],
        "the harness durable must filter exactly the coordinate the lib renders"
    );

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_widened_durable_is_converged_back_to_the_exact_filter() {
    let coords = accepted_event_coords().expect("accepted event coords");
    let exact = event_subject(&coords);
    let (harness, marker) = FabricTestNats::start()
        .await
        .with_widened_durable(INTEGRATION_EVT, "greedy", "integration.evt.>")
        .await;
    let WidenedDurable { stream, durable } = marker;

    harness
        .fabric()
        .ensure_event_durable(&coords, &durable)
        .await
        .expect("ensure_event_durable converges a widened durable back to the coordinate filter");

    let filters = harness.durable_filter_subjects(stream, &durable).await;
    assert_eq!(
        filters,
        vec![exact],
        "the widened durable must be narrowed back to the exact coordinate"
    );
    assert!(
        !filters.iter().any(|f| f == "integration.evt.>"),
        "the durable must no longer be widened on integration.evt.>"
    );

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
async fn a_missing_fixed_stream_makes_the_lib_probe_fail_loud() {
    let bare = BareFabricNats::with_only_event_stream().await;
    let coords = declare_command_coords().expect("declare command coords");

    let err = bare.assert_missing_command_stream(&coords, "absent").await;
    assert!(matches!(err, FabricError::Consume { .. }));
    assert!(bare.command_stream_absent().await);

    bare.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_missing_fixed_stream_makes_the_lib_bind_fail_loud() {
    let bare = BareFabricNats::with_only_event_stream().await;
    let coords = declare_command_coords().expect("declare command coords");

    let err = bare
        .assert_missing_command_stream_on_bind(&coords, "absent")
        .await;
    assert!(matches!(
        err,
        FabricError::Consume {
            kind: ConsumeErrorKind::NoStream,
            ..
        }
    ));
    assert!(
        bare.command_stream_absent().await,
        "the failed bind must not have created the fixed command stream"
    );

    bare.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn the_published_language_bucket_is_absent_until_opted_in() {
    let bare = BareFabricNats::without_fixed_streams().await;
    assert!(bare.published_language_absent().await);
    bare.shutdown().await;
}
