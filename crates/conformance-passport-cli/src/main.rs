mod cli;
mod duration;
mod error;
mod render;
mod report;
mod run;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command, OutputArgs};
use crate::error::{CliError, Result};
use crate::report::SubjectReport;

#[tokio::main]
async fn main() -> ExitCode {
    match dispatch().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
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
            let report = run::run_single(args).await?;
            emit(&report, &args.output)
        }
    }
}

fn emit(report: &SubjectReport, output: &OutputArgs) -> Result<bool> {
    let rendered = report.render(output.format);
    match &output.output {
        Some(path) => std::fs::write(path, rendered)
            .map_err(|e| CliError::new(format!("writing report to {}: {e}", path.display())))?,
        None => print!("{rendered}"),
    }
    Ok(report.conformant)
}
