use conformance_scope::ConformanceError;

pub fn invalid_input(
    result: Result<conformance_scope::ConformanceReport, ConformanceError>,
) -> String {
    match result {
        Err(ConformanceError::InvalidInput(detail)) => detail,
        other => panic!("expected InvalidInput: {other:?}"),
    }
}
