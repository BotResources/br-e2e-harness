#[path = "common/invalid_input.rs"]
mod invalid_input;
#[path = "common/short_timeout.rs"]
mod short_timeout;
#[path = "common/uniform.rs"]
mod uniform;

use conformance_scope::{AcceptorBehavior, AttachTarget, ConformanceError, Scenario, run_attach};
use invalid_input::invalid_input;
use short_timeout::TIMEOUT;
use uniform::uniform;

#[test]
fn a_readyz_url_that_is_not_an_http_url_is_invalid_input_naming_it() {
    for raw in [
        "127.0.0.1:8080/readyz",
        "not-a-url",
        "ftp://127.0.0.1/readyz",
        "http://",
    ] {
        match conformance_scope::ReadyzProbe::new(raw) {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains(raw), "{detail}");
            }
            Err(other) => panic!("`{raw}`: expected InvalidInput: {other:?}"),
            Ok(_) => panic!("`{raw}` must be refused"),
        }
    }
}

#[test]
fn a_readyz_url_with_an_http_or_https_host_is_accepted() {
    for raw in ["http://127.0.0.1:8080/readyz", "https://svc.example/readyz"] {
        assert!(conformance_scope::ReadyzProbe::new(raw).is_ok(), "{raw}");
    }
}

#[tokio::test]
async fn attach_with_a_scheme_less_readyz_url_is_invalid_input_before_touching_the_broker() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "127.0.0.1:8080/readyz".to_string(),
    };
    let detail = invalid_input(
        run_attach(
            &target,
            &uniform(),
            &AcceptorBehavior::Accept,
            &[Scenario::DeclareWellFormed],
            TIMEOUT,
        )
        .await,
    );
    assert!(detail.contains("127.0.0.1:8080/readyz"), "{detail}");
}
