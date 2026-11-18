use super::*;

#[test]
fn every_code_round_trips_and_is_unique() {
    for id in CheckId::ALL {
        assert_eq!(CheckId::from_code(id.code()), Some(id));
    }
    let mut codes: Vec<_> = CheckId::ALL.iter().map(|id| id.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), CheckId::ALL.len());
}

#[test]
fn attach_runs_everything_but_the_throwaway_server_checks() {
    let attach = attach_default();
    assert_eq!(attach.len(), CheckId::ALL.len() - 2);
    assert!(attach.iter().all(|id| !id.needs_throwaway_server()));
    assert_eq!(spawn_default().len(), CheckId::ALL.len());
}

#[test]
fn an_unknown_check_is_a_typed_error_naming_it() {
    let err = parse_checks("dead-grammar-fails-loud, nope").unwrap_err();
    assert!(matches!(err, ConformanceError::UnknownCheck(code) if code == "nope"));
}

#[test]
fn a_report_with_a_failure_is_not_conformant_and_a_skip_is() {
    let mut report = ConformanceReport::default();
    report
        .outcomes
        .push(CheckOutcome::pass(CheckId::DeadGrammarFailsLoud));
    report.outcomes.push(CheckOutcome::skipped(
        CheckId::MissingStreamFailsCoverageProbe,
        "needs a throwaway server",
    ));
    assert!(report.is_conformant());
    report.outcomes.push(CheckOutcome::fail(
        CheckId::WidenedDurableConverges,
        "still widened",
    ));
    assert!(!report.is_conformant());
    assert_eq!(
        (report.passed(), report.failed(), report.skipped()),
        (1, 1, 1)
    );
}

#[test]
fn a_repeated_check_id_is_a_typed_error_naming_it() {
    let err =
        parse_checks("dead-grammar-fails-loud, widened-durable-converges,dead-grammar-fails-loud")
            .unwrap_err();
    assert!(
        matches!(err, ConformanceError::DuplicateCheck(code) if code == "dead-grammar-fails-loud")
    );
    assert_eq!(
        parse_checks("dead-grammar-fails-loud,widened-durable-converges")
            .unwrap()
            .len(),
        2
    );
}
