mod cli;
mod duration;
mod error;
mod render;
mod report;
mod run;

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use crate::cli::{Cli, Command, OutputArgs, RunArgs};
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
    match cli.command {
        Command::Run(args) => {
            let args: Arc<RunArgs> = Arc::from(args);
            let shared = Arc::clone(&args);
            let report = contain_panics(async move { run::run_single(&shared).await }).await?;
            emit(&report, &args.output)
        }
    }
}

async fn contain_panics<T, F>(work: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    std::panic::set_hook(Box::new(|_| {}));
    match tokio::spawn(work).await {
        Ok(outcome) => outcome,
        Err(join) if join.is_panic() => Err(CliError::new(panic_message(join.into_panic()))),
        Err(join) => Err(CliError::new(format!("the run was cancelled: {join}"))),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "the run aborted on an unexpected internal error".to_string(),
        },
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
