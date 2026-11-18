use std::io::Write;

use crate::error::{CliError, Result};

pub fn write_stdout(bytes: &[u8]) -> Result<()> {
    write_all_flushed(&mut std::io::stdout().lock(), bytes)
        .map_err(|e| CliError::new(format!("writing to stdout: {e}")))
}

pub fn write_stderr_line(line: std::fmt::Arguments<'_>) {
    let _ = writeln!(std::io::stderr().lock(), "{line}");
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
