mod cli;
mod duration;
mod error;
mod render;
mod report;
mod run;

use std::io::Write;
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
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let joined = tokio::spawn(work).await;
    std::panic::set_hook(previous_hook);
    match joined {
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
        None => write_all_flushed(&mut std::io::stdout().lock(), rendered.as_bytes())
            .map_err(|e| CliError::new(format!("writing report to stdout: {e}")))?,
    }
    Ok(report.conformant)
}

fn write_all_flushed(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::io::{self, ErrorKind, Write};

    use super::write_all_flushed;

    struct BrokenPipe;

    impl Write for BrokenPipe {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FailingFlush;

    impl Write for FailingFlush {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(ErrorKind::StorageFull))
        }
    }

    #[test]
    fn a_broken_pipe_is_returned_as_an_error_instead_of_panicking() {
        let error = write_all_flushed(&mut BrokenPipe, b"report").expect_err("broken pipe");
        assert_eq!(error.kind(), ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_failing_flush_is_returned_as_an_error() {
        let error = write_all_flushed(&mut FailingFlush, b"report").expect_err("flush");
        assert_eq!(error.kind(), ErrorKind::StorageFull);
    }

    #[test]
    fn a_healthy_writer_receives_every_byte() {
        let mut sink = Vec::new();
        write_all_flushed(&mut sink, b"report").expect("write");
        assert_eq!(sink, b"report");
    }
}
