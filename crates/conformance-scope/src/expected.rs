use br_core_scope::ScopeDeclaration;

use crate::error::{ConformanceError, Result};

pub const SAMPLE_FALLBACK_SCOPE: &str = "example:read";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedScope {
    pub key: String,
    pub platform_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedDeclaration {
    pub service_key: String,
    pub scopes: Vec<ExpectedScope>,
}

impl ExpectedDeclaration {
    pub fn new(service_key: impl Into<String>, scopes: Vec<ExpectedScope>) -> Self {
        Self {
            service_key: service_key.into(),
            scopes,
        }
    }

    pub fn from_parts(
        service_key: impl Into<String>,
        scope_keys: &[String],
        platform_only: &PlatformOnly,
    ) -> Self {
        let scopes = scope_keys
            .iter()
            .map(|key| ExpectedScope {
                platform_only: platform_only.applies_to(key),
                key: key.clone(),
            })
            .collect();
        Self::new(service_key, scopes)
    }

    pub fn scope_keys_csv(&self) -> String {
        self.scopes
            .iter()
            .map(|s| s.key.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn assert_matches(&self, declared: &ScopeDeclaration) -> std::result::Result<(), String> {
        let declared_service = declared.manifest().key.as_str();
        if declared_service != self.service_key {
            return Err(format!(
                "service key mismatch: expected {:?}, observed {:?}",
                self.service_key, declared_service
            ));
        }

        let observed: Vec<ExpectedScope> = declared
            .scopes()
            .iter()
            .map(|s| ExpectedScope {
                key: s.key.as_str().to_string(),
                platform_only: s.platform_only,
            })
            .collect();

        if observed != self.scopes {
            return Err(format!(
                "scope set mismatch:\n  expected: {}\n  observed: {}",
                render_scopes(&self.scopes),
                render_scopes(&observed)
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum PlatformOnly {
    All(bool),
    PerScope(Vec<(String, bool)>),
}

impl PlatformOnly {
    pub fn applies_to(&self, scope_key: &str) -> bool {
        match self {
            PlatformOnly::All(value) => *value,
            PlatformOnly::PerScope(entries) => entries
                .iter()
                .find(|(key, _)| key == scope_key)
                .map(|(_, value)| *value)
                .unwrap_or(false),
        }
    }
}

pub fn parse_platform_only(raw: &str) -> Result<PlatformOnly> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(PlatformOnly::All(false));
    }
    if let Ok(value) = trimmed.parse::<bool>() {
        return Ok(PlatformOnly::All(value));
    }

    let mut entries = Vec::new();
    for part in trimmed.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once('=').ok_or_else(|| {
            ConformanceError::InvalidInput(format!(
                "platform-only entry {part:?} must be `true`, `false`, or `key=bool`"
            ))
        })?;
        let value = value.trim().parse::<bool>().map_err(|_| {
            ConformanceError::InvalidInput(format!(
                "platform-only value for {key:?} must be `true` or `false`, got {value:?}"
            ))
        })?;
        entries.push((key.trim().to_string(), value));
    }
    Ok(PlatformOnly::PerScope(entries))
}

pub fn parse_scope_keys(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn render_scopes(scopes: &[ExpectedScope]) -> String {
    if scopes.is_empty() {
        return "[]".to_string();
    }
    scopes
        .iter()
        .map(|s| format!("{}(platform_only={})", s.key, s.platform_only))
        .collect::<Vec<_>>()
        .join(", ")
}
