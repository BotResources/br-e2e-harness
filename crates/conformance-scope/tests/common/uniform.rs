use conformance_scope::{ExpectedDeclaration, ExpectedScope};

pub fn uniform() -> ExpectedDeclaration {
    ExpectedDeclaration::new(
        "svc",
        vec![ExpectedScope {
            key: "svc:read".to_string(),
            platform_only: false,
        }],
    )
}
