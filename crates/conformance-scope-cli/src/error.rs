use std::fmt;

#[derive(Debug)]
pub struct CliError(pub String);

impl CliError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CliError {}

impl From<conformance_scope::ConformanceError> for CliError {
    fn from(e: conformance_scope::ConformanceError) -> Self {
        Self(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
