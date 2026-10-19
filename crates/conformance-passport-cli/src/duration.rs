use std::time::Duration;

use crate::error::{CliError, Result};

pub fn parse(raw: &str) -> Result<Duration> {
    let trimmed = raw.trim();
    let (value, unit) = split_unit(trimmed);
    let amount: u64 = value.parse().map_err(|_| {
        CliError::new(format!(
            "invalid duration {raw:?}: expected e.g. 10s, 500ms"
        ))
    })?;
    match unit {
        "ms" => Ok(Duration::from_millis(amount)),
        "s" | "" => Ok(Duration::from_secs(amount)),
        "m" => Ok(Duration::from_secs(amount * 60)),
        other => Err(CliError::new(format!(
            "invalid duration unit {other:?} in {raw:?}: use ms, s, or m"
        ))),
    }
}

fn split_unit(trimmed: &str) -> (&str, &str) {
    let boundary = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    trimmed.split_at(boundary)
}
