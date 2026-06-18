use br_core_integration::{Aggregate, Bc, CommandCoords, CoordError, EventCoords, PastFact, Verb};
use br_util_nats_fabric::{command_subject, event_subject};
use serde::Deserialize;

#[derive(thiserror::Error, Debug)]
pub enum ManifestError {
    #[error("reading manifest '{path}': {detail}")]
    Read { path: String, detail: String },
    #[error("parsing manifest '{path}': {detail}")]
    Parse { path: String, detail: String },
    #[error("invalid coordinate in manifest: {0}")]
    Coord(#[from] CoordError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub command_durable: Vec<CommandDurableSpec>,
    #[serde(default)]
    pub event_durable: Vec<EventDurableSpec>,
    #[serde(default)]
    pub published_language: PublishedLanguageSpec,
    #[serde(default)]
    pub bearer_tokens: BearerTokensSpec,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDurableSpec {
    pub durable: String,
    pub receiver: String,
    pub aggregate: String,
    pub verb: String,
    pub version: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDurableSpec {
    pub durable: String,
    pub producer: String,
    pub aggregate: String,
    pub fact: String,
    pub version: u8,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedLanguageSpec {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BearerTokensSpec {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug)]
pub struct RenderedCommand {
    pub durable: String,
    pub coords: CommandCoords,
    pub subject: String,
}

#[derive(Debug)]
pub struct RenderedEvent {
    pub durable: String,
    pub coords: EventCoords,
    pub subject: String,
}

#[derive(Debug)]
pub struct Rendered {
    pub commands: Vec<RenderedCommand>,
    pub events: Vec<RenderedEvent>,
    pub published_language: bool,
    pub bearer_tokens: bool,
}

impl Manifest {
    pub fn parse(path: &str) -> Result<Self, ManifestError> {
        let raw = std::fs::read_to_string(path).map_err(|e| ManifestError::Read {
            path: path.to_string(),
            detail: e.to_string(),
        })?;
        toml::from_str(&raw).map_err(|e| ManifestError::Parse {
            path: path.to_string(),
            detail: e.to_string(),
        })
    }

    pub fn render(&self, run_id: Option<&str>) -> Result<Rendered, ManifestError> {
        let mut commands = Vec::new();
        for spec in &self.command_durable {
            let coords = CommandCoords {
                receiver: Bc::new(spec.receiver.clone())?,
                aggregate: Aggregate::new(spec.aggregate.clone())?,
                verb: Verb::new(spec.verb.clone())?,
                version: spec.version,
            };
            let subject = command_subject(&coords);
            commands.push(RenderedCommand {
                durable: durable_name(&spec.durable, run_id),
                coords,
                subject,
            });
        }

        let mut events = Vec::new();
        for spec in &self.event_durable {
            let coords = EventCoords {
                producer: Bc::new(spec.producer.clone())?,
                aggregate: Aggregate::new(spec.aggregate.clone())?,
                fact: PastFact::new(spec.fact.clone())?,
                version: spec.version,
            };
            let subject = event_subject(&coords);
            events.push(RenderedEvent {
                durable: durable_name(&spec.durable, run_id),
                coords,
                subject,
            });
        }

        Ok(Rendered {
            commands,
            events,
            published_language: self.published_language.enabled,
            bearer_tokens: self.bearer_tokens.enabled,
        })
    }
}

fn durable_name(logical: &str, run_id: Option<&str>) -> String {
    match run_id {
        Some(run_id) => format!("{logical}_{run_id}"),
        None => logical.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[command_durable]]
durable = "declare_worker"
receiver = "identity"
aggregate = "service_scope"
verb = "declare"
version = 1

[[event_durable]]
durable = "user_projector"
producer = "identity"
aggregate = "user"
fact = "created"
version = 1

[published_language]
enabled = true

[bearer_tokens]
enabled = true
"#;

    fn parse(raw: &str) -> Manifest {
        toml::from_str(raw).expect("manifest parses")
    }

    #[test]
    fn renders_coords_into_the_fixed_grammar() {
        let rendered = parse(SAMPLE).render(None).unwrap();
        assert_eq!(
            rendered.commands[0].subject,
            "integration.cmd.identity.service_scope.declare.v1"
        );
        assert_eq!(
            rendered.events[0].subject,
            "integration.evt.identity.user.created.v1"
        );
        assert!(rendered.published_language);
        assert!(rendered.bearer_tokens);
    }

    #[test]
    fn durable_is_literal_without_a_run_id() {
        let rendered = parse(SAMPLE).render(None).unwrap();
        assert_eq!(rendered.commands[0].durable, "declare_worker");
    }

    #[test]
    fn durable_is_suffixed_with_the_run_id() {
        let rendered = parse(SAMPLE).render(Some("abc123")).unwrap();
        assert_eq!(rendered.commands[0].durable, "declare_worker_abc123");
        assert_eq!(rendered.events[0].durable, "user_projector_abc123");
    }

    #[test]
    fn a_raw_subject_key_is_rejected() {
        let raw = r#"
[[event_durable]]
durable = "x"
subject = "integration.evt.identity.user.created.v1"
producer = "identity"
aggregate = "user"
fact = "created"
version = 1
"#;
        assert!(toml::from_str::<Manifest>(raw).is_err());
    }

    #[test]
    fn an_invalid_coordinate_is_a_coord_error() {
        let raw = r#"
[[event_durable]]
durable = "x"
producer = "identity"
aggregate = "user.profile"
fact = "created"
version = 1
"#;
        let err = parse(raw).render(None).unwrap_err();
        assert!(matches!(err, ManifestError::Coord(_)));
    }

    #[test]
    fn an_empty_manifest_renders_nothing() {
        let rendered = parse("").render(None).unwrap();
        assert!(rendered.commands.is_empty());
        assert!(rendered.events.is_empty());
        assert!(!rendered.published_language);
        assert!(!rendered.bearer_tokens);
    }
}
