use std::time::Duration;

use crate::error::{ConformanceError, Result};

pub const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) fn validated_timeout(timeout: Duration) -> Result<Duration> {
    if timeout.is_zero() {
        return Err(ConformanceError::InvalidInput(format!(
            "timeout {timeout:?} is not greater than zero; no service can answer within it"
        )));
    }
    if timeout > MAX_TIMEOUT {
        return Err(ConformanceError::InvalidInput(format!(
            "timeout {timeout:?} is above the cap of {MAX_TIMEOUT:?} (24 hours)"
        )));
    }
    Ok(timeout)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{MAX_TIMEOUT, validated_timeout};
    use crate::error::ConformanceError;

    #[test]
    fn given_a_timeout_within_the_cap_when_validated_then_it_is_returned_unchanged() {
        for timeout in [
            Duration::from_millis(1),
            Duration::from_secs(10),
            MAX_TIMEOUT,
        ] {
            assert_eq!(validated_timeout(timeout).unwrap(), timeout);
        }
    }

    #[test]
    fn given_zero_or_a_value_above_the_cap_when_validated_then_input_is_refused_naming_the_value() {
        for timeout in [
            Duration::ZERO,
            MAX_TIMEOUT + Duration::from_nanos(1),
            Duration::from_secs(u64::MAX),
            Duration::MAX,
        ] {
            match validated_timeout(timeout) {
                Err(ConformanceError::InvalidInput(detail)) => {
                    assert!(detail.contains(&format!("{timeout:?}")), "{detail}");
                }
                other => panic!("{timeout:?}: expected InvalidInput: {other:?}"),
            }
        }
    }
}
