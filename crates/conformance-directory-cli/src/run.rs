use conformance_directory::{
    ALL_CHECKS, emit_snapshot, locate_and_emit, parse_checks, run_battery,
};

use crate::cli::RunArgs;
use crate::error::Result;
use crate::report::SubjectReport;

pub async fn run_single(args: &RunArgs) -> Result<SubjectReport> {
    let checks = match &args.checks {
        Some(raw) => parse_checks(raw)?,
        None => ALL_CHECKS.to_vec(),
    };
    let snapshot = match &args.anchor_binary {
        Some(binary) => emit_snapshot(binary).await?,
        None => locate_and_emit().await?,
    };
    let report = run_battery(&snapshot, &checks).await?;
    Ok(SubjectReport::from_report(
        args.subject_label.clone(),
        &report,
    ))
}
