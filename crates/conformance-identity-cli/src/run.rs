use std::path::Path;

use conformance_identity::{
    AttachTarget, ConformanceReport, SUBJECT_BINARY, Scenario, SpawnTarget, attach_default,
    locate_subject, parse_scenarios, run_attach, run_spawn, spawn_default,
};

use crate::cli::RunArgs;
use crate::duration;
use crate::error::{CliError, Result};
use crate::report::SubjectReport;

enum Mode<'a> {
    Spawn(&'a Path),
    Anchor,
    Attach { nats: &'a str, readyz: &'a str },
}

pub async fn run_single(args: &RunArgs) -> Result<SubjectReport> {
    let timeout = duration::parse(&args.timeout)?;
    let mode = mode_of(args)?;
    let scenarios = resolve_scenarios(args, &mode)?;
    let (label, report): (String, ConformanceReport) = match mode {
        Mode::Spawn(binary) => {
            let label = label_or(args, || default_label(binary));
            let target = SpawnTarget {
                binary: binary.to_path_buf(),
            };
            (label, run_spawn(&target, &scenarios, timeout).await?)
        }
        Mode::Anchor => {
            let binary = locate_subject().await?;
            let label = label_or(args, || SUBJECT_BINARY.to_string());
            let target = SpawnTarget { binary };
            (label, run_spawn(&target, &scenarios, timeout).await?)
        }
        Mode::Attach { nats, readyz } => {
            let label = label_or(args, || readyz.to_string());
            let target = AttachTarget {
                nats_url: nats.to_string(),
                readyz_url: readyz.to_string(),
            };
            (label, run_attach(&target, &scenarios, timeout).await?)
        }
    };
    Ok(SubjectReport::from_report(label, &report))
}

fn mode_of(args: &RunArgs) -> Result<Mode<'_>> {
    match (&args.spawn, args.anchor, &args.nats, &args.readyz) {
        (Some(binary), _, _, _) => Ok(Mode::Spawn(binary)),
        (None, true, _, _) => Ok(Mode::Anchor),
        (None, false, Some(nats), Some(readyz)) => Ok(Mode::Attach { nats, readyz }),
        _ => Err(CliError::new(
            "choose one of --spawn, --anchor or --attach (--attach needs --nats and --readyz)",
        )),
    }
}

fn resolve_scenarios(args: &RunArgs, mode: &Mode<'_>) -> Result<Vec<Scenario>> {
    match &args.scenarios {
        Some(raw) => Ok(parse_scenarios(raw)?),
        None => Ok(match mode {
            Mode::Attach { .. } => attach_default(),
            Mode::Spawn(_) | Mode::Anchor => spawn_default(),
        }),
    }
}

fn label_or(args: &RunArgs, default: impl FnOnce() -> String) -> String {
    args.subject_label.clone().unwrap_or_else(default)
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
