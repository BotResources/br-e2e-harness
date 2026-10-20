mod cli;
mod duration;
mod error;
mod manifest;
mod render;
mod report;
mod run;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command, OutputArgs};
use crate::error::{CliError, Result};
use crate::report::AggregateReport;

#[tokio::main]
async fn main() -> ExitCode {
    match dispatch().await {
        Ok(conformant) => {
            if conformant {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

async fn dispatch() -> Result<bool> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Run(args) => {
            let service = run::run_single(args).await?;
            emit(AggregateReport::single(service), &args.output)
        }
        Command::Manifest(args) => {
            let raw = std::fs::read_to_string(&args.file).map_err(|e| {
                CliError::new(format!("reading manifest {}: {e}", args.file.display()))
            })?;
            let manifest = manifest::load(&raw)?;
            let run = manifest::run(&manifest).await?;
            match run.failure {
                None => emit(run.report, &args.output),
                Some(failure) => {
                    if !run.report.services.is_empty() {
                        emit(run.report, &args.output)?;
                    }
                    Err(failure)
                }
            }
        }
    }
}

fn emit(report: AggregateReport, output: &OutputArgs) -> Result<bool> {
    let rendered = report.render(output.format);
    match &output.output {
        Some(path) => std::fs::write(path, rendered)
            .map_err(|e| CliError::new(format!("writing report to {}: {e}", path.display())))?,
        None => print!("{rendered}"),
    }
    Ok(report.conformant)
}
