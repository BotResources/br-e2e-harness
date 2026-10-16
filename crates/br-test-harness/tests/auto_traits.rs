#![cfg(feature = "full")]

use std::panic::{RefUnwindSafe, UnwindSafe};

use br_test_harness::fabric_nats::{
    BareFabricNats, BearerSeeder, CapturedMessage, CommandAwaiter, CommandCapture, DeliveryOutage,
    DurableConfig, DurableTap, EventCapture, FabricAwaiter, FabricKvError, FabricTestNats,
    FixedStream, Manifest, ManifestError, NatsBacking, Rendered, RenderedCommand, RenderedEvent,
    RunNamespace, SeededToken, TapOutcome, TapStop, TappedDelivery, WidenedDurable,
};
use br_test_harness::{
    E2eDatabase, GraphqlClient, SpawnedNats, SpawnedProcess, SseSubscription, TestNats, TestServer,
    WsSubscription,
};

fn sendable<T: Send + Sync + Unpin>() {}
fn movable<T: Send + Unpin>() {}
fn unwind_tolerant<T: UnwindSafe + RefUnwindSafe>() {}

#[test]
fn every_public_handle_keeps_the_auto_traits_it_had_in_0_1_1() {
    sendable::<BareFabricNats>();
    sendable::<BearerSeeder>();
    sendable::<CapturedMessage>();
    sendable::<CommandAwaiter>();
    sendable::<CommandCapture>();
    sendable::<DeliveryOutage>();
    sendable::<DurableConfig>();
    sendable::<DurableTap>();
    sendable::<EventCapture>();
    sendable::<FabricAwaiter>();
    sendable::<FabricKvError>();
    sendable::<FabricTestNats>();
    sendable::<FixedStream>();
    sendable::<Manifest>();
    sendable::<ManifestError>();
    sendable::<NatsBacking>();
    sendable::<Rendered>();
    sendable::<RenderedCommand>();
    sendable::<RenderedEvent>();
    sendable::<RunNamespace>();
    sendable::<SeededToken>();
    sendable::<TapOutcome>();
    sendable::<TapStop>();
    sendable::<TappedDelivery>();
    sendable::<WidenedDurable>();
    sendable::<E2eDatabase>();
    sendable::<GraphqlClient>();
    sendable::<SpawnedNats>();
    sendable::<SpawnedProcess>();
    sendable::<TestNats>();
    sendable::<TestServer>();
    sendable::<WsSubscription>();
    movable::<SseSubscription>();
}

#[test]
fn the_captures_and_the_command_awaiter_stay_unwind_safe() {
    unwind_tolerant::<CommandAwaiter>();
    unwind_tolerant::<CommandCapture>();
    unwind_tolerant::<EventCapture>();
    unwind_tolerant::<CapturedMessage>();
}
