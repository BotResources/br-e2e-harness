mod cli;
mod duration;
mod error;
mod output;
mod panic_hook;
mod render;
mod report;

use std::process::ExitCode;

use clap::Parser;
use conformance_nats_fabric::{
    AttachTarget, CheckId, attach_default, parse_checks, run_attach, run_spawn, spawn_default,
};

use crate::cli::{Cli, Command, RunArgs};
use crate::error::{CliError, Result};
use crate::output::{write_stderr_line, write_stdout};
use crate::panic_hook::PanicHookGuard;
use crate::report::Report;

#[tokio::main]
async fn main() -> ExitCode {
    match dispatch().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            write_stderr_line(format_args!("error: {e}"));
            ExitCode::from(2)
        }
    }
}

async fn dispatch() -> Result<bool> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(args) => run(&args).await,
        Command::List => {
            list()?;
            Ok(true)
        }
    }
}

fn list() -> Result<()> {
    let mut text = String::new();
    for id in CheckId::ALL {
        let scope = if id.needs_throwaway_server() {
            "spawn only"
        } else {
            "spawn, attach"
        };
        text.push_str(&format!(
            "{}  [{scope}]\n    {}\n",
            id.code(),
            id.expectation()
        ));
    }
    write_stdout(text.as_bytes())
}

async fn run(args: &RunArgs) -> Result<bool> {
    let timeout = duration::parse(&args.timeout)?;
    let hook = PanicHookGuard::install();
    let report = match &args.nats {
        Some(url) => {
            let checks = match &args.checks {
                Some(raw) => parse_checks(raw)?,
                None => attach_default(),
            };
            let target = AttachTarget {
                nats_url: url.clone(),
            };
            let report = run_attach(&target, &checks, timeout).await?;
            Report::attached(url, &report)
        }
        None => {
            let checks = match &args.checks {
                Some(raw) => parse_checks(raw)?,
                None => spawn_default(),
            };
            let report = run_spawn(&checks, timeout).await?;
            Report::spawned(&report)
        }
    };
    drop(hook);
    let rendered = report.render(args.format);
    match &args.output {
        Some(path) => std::fs::write(path, rendered)
            .map_err(|e| CliError::new(format!("writing report to {}: {e}", path.display())))?,
        None => write_stdout(rendered.as_bytes())?,
    }
    Ok(report.conformant)
}
