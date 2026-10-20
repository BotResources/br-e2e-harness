use std::path::Path;

use conformance_passport::{
    ALL, ConformanceReport, Scenario, SpawnTarget, locate_subject, parse_scenarios, run_spawn,
};

use crate::cli::RunArgs;
use crate::duration;
use crate::error::Result;
use crate::report::SubjectReport;

pub async fn run_single(args: &RunArgs) -> Result<SubjectReport> {
    let timeout = duration::parse(&args.timeout)?;
    let scenarios = resolve_scenarios(args)?;
    let binary = match &args.spawn {
        Some(binary) => binary.clone(),
        None => locate_subject().await?,
    };
    let label = args
        .subject_label
        .clone()
        .unwrap_or_else(|| default_label(&binary));
    let report: ConformanceReport = run_spawn(&SpawnTarget { binary }, &scenarios, timeout).await?;
    Ok(SubjectReport::from_report(label, &report))
}

fn resolve_scenarios(args: &RunArgs) -> Result<Vec<Scenario>> {
    match &args.scenarios {
        Some(raw) => Ok(parse_scenarios(raw)?),
        None => Ok(ALL.to_vec()),
    }
}

fn default_label(binary: &Path) -> String {
    binary
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| binary.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn the_default_label_is_the_file_name_of_the_binary() {
        assert_eq!(
            default_label(&PathBuf::from("/opt/bin/my-service")),
            "my-service"
        );
        assert_eq!(default_label(&PathBuf::from("my-service")), "my-service");
    }
}
