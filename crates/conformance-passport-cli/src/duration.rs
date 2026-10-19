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
        "m" => amount
            .checked_mul(60)
            .map(Duration::from_secs)
            .ok_or_else(|| CliError::new(format!("invalid duration {raw:?}: value too large"))),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn message(raw: &str) -> String {
        parse(raw).expect_err("must be rejected").to_string()
    }

    #[test]
    fn milliseconds_seconds_and_minutes_parse() {
        assert_eq!(parse("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse("2m").unwrap(), Duration::from_secs(120));
    }

    #[test]
    fn a_bare_number_is_seconds() {
        assert_eq!(parse("7").unwrap(), Duration::from_secs(7));
        assert_eq!(parse(" 7 ").unwrap(), Duration::from_secs(7));
    }

    #[test]
    fn a_bad_unit_is_rejected() {
        assert!(message("5h").contains("invalid duration unit"));
    }

    #[test]
    fn garbage_and_empty_text_are_rejected() {
        assert!(message("soon").contains("invalid duration"));
        assert!(message("").contains("invalid duration"));
        assert!(message("-5s").contains("invalid duration"));
    }

    #[test]
    fn minutes_that_overflow_are_rejected_not_wrapped() {
        assert!(message("307445734561825861m").contains("value too large"));
        assert!(message("99999999999999999999s").contains("invalid duration"));
    }
}
