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
    let duration = match unit {
        "ms" => Some(Duration::from_millis(amount)),
        "s" | "" => Some(Duration::from_secs(amount)),
        "m" => amount.checked_mul(60).map(Duration::from_secs),
        other => {
            return Err(CliError::new(format!(
                "invalid duration unit {other:?} in {raw:?}: use ms, s, or m"
            )));
        }
    };
    duration.ok_or_else(|| CliError::new(format!("invalid duration {raw:?}: value is too large")))
}

fn split_unit(trimmed: &str) -> (&str, &str) {
    let boundary = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    trimmed.split_at(boundary)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::parse;

    #[test]
    fn given_valid_durations_when_parsed_then_each_unit_is_read() {
        assert_eq!(parse("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse("10").unwrap(), Duration::from_secs(10));
        assert_eq!(parse(" 2m ").unwrap(), Duration::from_secs(120));
    }

    #[test]
    fn given_a_zero_duration_when_parsed_then_it_is_passed_on_for_the_battery_to_judge() {
        for raw in ["0", "0s", "0ms", "0m", "00s"] {
            assert_eq!(parse(raw).unwrap(), Duration::ZERO, "{raw}");
        }
    }

    #[test]
    fn given_minutes_that_overflow_when_parsed_then_it_is_refused_without_panicking() {
        let error = parse("307445734561825862m").expect_err("overflow");

        assert!(error.to_string().contains("too large"), "{error}");
    }

    #[test]
    fn given_seconds_that_fill_a_u64_when_parsed_then_they_are_passed_on_for_the_battery_to_judge()
    {
        assert_eq!(
            parse("18446744073709551615s").unwrap(),
            Duration::from_secs(u64::MAX)
        );
    }

    #[test]
    fn given_a_long_but_realistic_duration_when_parsed_then_it_is_accepted() {
        assert_eq!(parse("1440m").unwrap(), Duration::from_secs(86_400));
        assert_eq!(parse("3600s").unwrap(), Duration::from_secs(3_600));
    }

    #[test]
    fn given_malformed_durations_when_parsed_then_they_are_refused() {
        for raw in ["", "s", "-1s", "1.5s", "10h", "ten"] {
            assert!(parse(raw).is_err(), "{raw}");
        }
    }
}
