use std::time::Duration;

use crate::error::{CliError, Result};

pub fn parse(raw: &str) -> Result<Duration> {
    let trimmed = raw.trim();
    let (value, unit) = split_unit(trimmed);
    let invalid = || {
        CliError::new(format!(
            "invalid duration {raw:?}: expected e.g. 10s, 500ms"
        ))
    };
    let amount: u64 = value.parse().map_err(|_| invalid())?;
    let parsed = match unit {
        "ms" => Some(Duration::from_millis(amount)),
        "s" | "" => Some(Duration::from_secs(amount)),
        "m" => amount.checked_mul(60).map(Duration::from_secs),
        other => {
            return Err(CliError::new(format!(
                "invalid duration unit {other:?} in {raw:?}: use ms, s, or m"
            )));
        }
    };
    match parsed {
        Some(duration) if !duration.is_zero() => Ok(duration),
        Some(_) => Err(CliError::new(format!(
            "invalid duration {raw:?}: a timeout of zero lets no check finish"
        ))),
        None => Err(CliError::new(format!(
            "invalid duration {raw:?}: too large"
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

    #[test]
    fn the_documented_units_parse() {
        assert_eq!(parse("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse("10").unwrap(), Duration::from_secs(10));
        assert_eq!(parse("2m").unwrap(), Duration::from_secs(120));
    }

    #[test]
    fn a_minute_count_that_overflows_is_an_error_not_a_panic_or_a_wrap() {
        for raw in ["307445734561825861m", "18446744073709551615m"] {
            let error = parse(raw).expect_err(raw);
            assert!(error.to_string().contains("too large"), "{error}");
        }
    }

    #[test]
    fn the_largest_representable_minute_count_still_parses() {
        assert_eq!(
            parse("307445734561825860m").unwrap(),
            Duration::from_secs(307_445_734_561_825_860 * 60)
        );
    }

    #[test]
    fn a_zero_timeout_is_an_error_in_every_unit() {
        for raw in ["0", "0s", "0ms", "0m", " 0s "] {
            let error = parse(raw).expect_err(raw);
            assert!(error.to_string().contains("zero"), "{raw}: {error}");
        }
    }

    #[test]
    fn malformed_values_are_errors() {
        for raw in ["", "s", "-1s", "1.5s", "10h", "1 0s"] {
            assert!(parse(raw).is_err(), "{raw}");
        }
    }
}
