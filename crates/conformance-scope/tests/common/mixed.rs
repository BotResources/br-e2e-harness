use conformance_scope::{ExpectedDeclaration, PlatformOnly};

pub fn mixed() -> ExpectedDeclaration {
    ExpectedDeclaration::from_parts(
        "svc",
        &["svc:a".to_string(), "svc:b".to_string()],
        &PlatformOnly::PerScope(vec![("svc:a".to_string(), true)]),
    )
}
