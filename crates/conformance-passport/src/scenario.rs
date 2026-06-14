use crate::error::{ConformanceError, Result};
use crate::outcome::CheckId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    ValidBearerResolvesToPassport,
    RevokedBearerIsAnonymous,
    UnknownBearerIsAnonymous,
    NoCredentialIsAnonymous,
    DistinctTokensDistinctPassports,
}

pub const ALL: [Scenario; 5] = [
    Scenario::ValidBearerResolvesToPassport,
    Scenario::RevokedBearerIsAnonymous,
    Scenario::UnknownBearerIsAnonymous,
    Scenario::NoCredentialIsAnonymous,
    Scenario::DistinctTokensDistinctPassports,
];

impl Scenario {
    pub fn check_id(self) -> CheckId {
        match self {
            Scenario::ValidBearerResolvesToPassport => CheckId::ValidBearerResolvesToPassport,
            Scenario::RevokedBearerIsAnonymous => CheckId::RevokedBearerIsAnonymous,
            Scenario::UnknownBearerIsAnonymous => CheckId::UnknownBearerIsAnonymous,
            Scenario::NoCredentialIsAnonymous => CheckId::NoCredentialIsAnonymous,
            Scenario::DistinctTokensDistinctPassports => CheckId::DistinctTokensDistinctPassports,
        }
    }

    pub fn code(self) -> &'static str {
        self.check_id().code()
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match CheckId::from_code(code)? {
            CheckId::ValidBearerResolvesToPassport => Some(Scenario::ValidBearerResolvesToPassport),
            CheckId::RevokedBearerIsAnonymous => Some(Scenario::RevokedBearerIsAnonymous),
            CheckId::UnknownBearerIsAnonymous => Some(Scenario::UnknownBearerIsAnonymous),
            CheckId::NoCredentialIsAnonymous => Some(Scenario::NoCredentialIsAnonymous),
            CheckId::DistinctTokensDistinctPassports => {
                Some(Scenario::DistinctTokensDistinctPassports)
            }
        }
    }
}

pub fn parse_scenarios(raw: &str) -> Result<Vec<Scenario>> {
    let mut scenarios = Vec::new();
    for part in raw.split(',') {
        let code = part.trim();
        if code.is_empty() {
            continue;
        }
        let scenario = Scenario::from_code(code)
            .ok_or_else(|| ConformanceError::InvalidInput(format!("unknown scenario {code:?}")))?;
        if !scenarios.contains(&scenario) {
            scenarios.push(scenario);
        }
    }
    if scenarios.is_empty() {
        return Err(ConformanceError::InvalidInput(
            "no scenarios selected".to_string(),
        ));
    }
    Ok(scenarios)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_round_trips_its_code() {
        for scenario in ALL {
            assert_eq!(Scenario::from_code(scenario.code()), Some(scenario));
        }
    }

    #[test]
    fn parse_scenarios_dedupes_and_rejects_unknown() {
        let parsed = parse_scenarios("p1, p1 ,p3").unwrap();
        assert_eq!(
            parsed,
            vec![
                Scenario::ValidBearerResolvesToPassport,
                Scenario::UnknownBearerIsAnonymous
            ]
        );
        assert!(parse_scenarios("p9").is_err());
        assert!(parse_scenarios("  ").is_err());
    }
}
