#[path = "common/absent_binary.rs"]
mod absent_binary;
#[path = "common/mixed.rs"]
mod mixed;
#[path = "common/short_timeout.rs"]
mod short_timeout;
#[path = "common/uniform.rs"]
mod uniform;

use std::time::Duration;

use absent_binary::absent_binary;
use conformance_scope::{
    AttachTarget, Scenario, SpawnTarget, check_attach_input, check_spawn_input,
};
use mixed::mixed;
use short_timeout::TIMEOUT;
use uniform::uniform;

#[test]
fn the_attach_input_check_refuses_what_run_attach_refuses_and_accepts_a_runnable_input() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let scenarios = [Scenario::DeclareWellFormed];

    assert!(check_attach_input(&target, &uniform(), &scenarios, TIMEOUT).is_ok());
    assert!(check_attach_input(&target, &uniform(), &[], TIMEOUT).is_err());
    assert!(check_attach_input(&target, &uniform(), &scenarios, Duration::ZERO).is_err());
    let bad_readyz = AttachTarget {
        nats_url: target.nats_url.clone(),
        readyz_url: "127.0.0.1:1/readyz".to_string(),
    };
    assert!(check_attach_input(&bad_readyz, &uniform(), &scenarios, TIMEOUT).is_err());
}

#[test]
fn the_spawn_input_check_refuses_what_run_spawn_refuses_and_accepts_a_runnable_input() {
    let scenarios = [Scenario::DeclareWellFormed];
    let present = SpawnTarget {
        binary: std::env::current_exe().expect("current exe"),
    };

    assert!(check_spawn_input(&present, &uniform(), &scenarios, TIMEOUT).is_ok());
    assert!(check_spawn_input(&present, &mixed(), &scenarios, TIMEOUT).is_err());
    assert!(check_spawn_input(&present, &uniform(), &[], TIMEOUT).is_err());
    assert!(check_spawn_input(&absent_binary(), &uniform(), &scenarios, TIMEOUT).is_err());
}
