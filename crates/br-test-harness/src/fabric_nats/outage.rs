use async_nats::jetstream::{self, stream};
use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{INTEGRATION_CMD, INTEGRATION_EVT, command_subject, event_subject};

use super::FabricTestNats;

const EVENT_PLACEHOLDER: &str = "integration.evt.__withheld__.>";
const COMMAND_PLACEHOLDER: &str = "integration.cmd.__withheld__.>";

#[must_use = "a DeliveryOutage rewrites a fixed stream's subjects; call restore() to put them back"]
pub struct DeliveryOutage {
    js: jetstream::Context,
    stream: &'static str,
    withheld: Vec<String>,
    live: Vec<String>,
    restored: Vec<String>,
}

impl DeliveryOutage {
    pub fn stream(&self) -> &'static str {
        self.stream
    }

    pub fn withheld_subjects(&self) -> &[String] {
        &self.withheld
    }

    pub fn live_subjects(&self) -> &[String] {
        &self.live
    }

    pub async fn restore(self) {
        let mut config = stream_config(&self.js, self.stream).await;
        config.subjects = self.restored;
        update_subjects(&self.js, config).await;
    }
}

impl FabricTestNats {
    pub async fn withhold_event_subject(
        &self,
        withheld: &EventCoords,
        keep: &[&EventCoords],
    ) -> DeliveryOutage {
        let keep = keep.iter().map(|coords| event_subject(coords)).collect();
        self.withhold_subject(
            INTEGRATION_EVT,
            event_subject(withheld),
            keep,
            EVENT_PLACEHOLDER,
        )
        .await
    }

    pub async fn withhold_command_subject(
        &self,
        withheld: &CommandCoords,
        keep: &[&CommandCoords],
    ) -> DeliveryOutage {
        let keep = keep.iter().map(|coords| command_subject(coords)).collect();
        self.withhold_subject(
            INTEGRATION_CMD,
            command_subject(withheld),
            keep,
            COMMAND_PLACEHOLDER,
        )
        .await
    }

    pub async fn withhold_event_stream(&self) -> DeliveryOutage {
        self.withhold_stream(INTEGRATION_EVT, EVENT_PLACEHOLDER)
            .await
    }

    pub async fn withhold_command_stream(&self) -> DeliveryOutage {
        self.withhold_stream(INTEGRATION_CMD, COMMAND_PLACEHOLDER)
            .await
    }

    async fn withhold_subject(
        &self,
        stream_name: &'static str,
        withheld: String,
        keep: Vec<String>,
        placeholder: &str,
    ) -> DeliveryOutage {
        let mut config = stream_config(&self.js, stream_name).await;
        let restored = config.subjects.clone();
        assert_covered(stream_name, &restored, &withheld);
        for (index, kept) in keep.iter().enumerate() {
            assert!(
                *kept != withheld,
                "withholding '{withheld}' on {stream_name}: the withheld coordinate is also \
                 listed in `keep`, which would ask the outage to both drop and keep it"
            );
            assert!(
                !keep[..index].contains(kept),
                "withholding '{withheld}' on {stream_name}: '{kept}' appears twice in `keep`, \
                 which the broker rejects as duplicate stream subjects"
            );
            assert_covered(stream_name, &restored, kept);
        }
        let live = if keep.is_empty() {
            vec![placeholder.to_string()]
        } else {
            keep
        };
        config.subjects = live.clone();
        update_subjects(&self.js, config).await;
        DeliveryOutage {
            js: self.js.clone(),
            stream: stream_name,
            withheld: vec![withheld],
            live,
            restored,
        }
    }

    async fn withhold_stream(
        &self,
        stream_name: &'static str,
        placeholder: &str,
    ) -> DeliveryOutage {
        let mut config = stream_config(&self.js, stream_name).await;
        let restored = config.subjects.clone();
        assert!(
            !restored.iter().any(|bound| bound == placeholder),
            "{stream_name} already binds the withheld placeholder {restored:?}: a whole-stream \
             outage nested inside another would record the placeholder as the binding to restore"
        );
        let live = vec![placeholder.to_string()];
        config.subjects = live.clone();
        update_subjects(&self.js, config).await;
        DeliveryOutage {
            js: self.js.clone(),
            stream: stream_name,
            withheld: restored.clone(),
            live,
            restored,
        }
    }
}

async fn stream_config(js: &jetstream::Context, stream_name: &'static str) -> stream::Config {
    let stream = js
        .get_stream(stream_name)
        .await
        .unwrap_or_else(|e| panic!("read fixed stream {stream_name} for a delivery outage: {e}"));
    stream.cached_info().config.clone()
}

async fn update_subjects(js: &jetstream::Context, config: stream::Config) {
    let stream_name = config.name.clone();
    let subjects = config.subjects.clone();
    js.update_stream(&config).await.unwrap_or_else(|e| {
        panic!("rewrite the subjects of fixed stream {stream_name} to {subjects:?}: {e}")
    });
}

fn assert_covered(stream_name: &'static str, subjects: &[String], subject: &str) {
    assert!(
        subjects.iter().any(|bound| covers(bound, subject)),
        "{stream_name} currently binds {subjects:?}, which does not cover '{subject}': a \
         delivery outage can only withhold a coordinate the stream carries right now"
    );
}

fn covers(pattern: &str, subject: &str) -> bool {
    if pattern.is_empty() || subject.is_empty() {
        return false;
    }
    let mut subject_tokens = subject.split('.');
    let mut pattern_tokens = pattern.split('.');
    while let Some(bound) = pattern_tokens.next() {
        match bound {
            ">" => return pattern_tokens.next().is_none() && subject_tokens.next().is_some(),
            "*" => {
                if subject_tokens.next().is_none() {
                    return false;
                }
            }
            literal => match subject_tokens.next() {
                Some(token) if token == literal => {}
                _ => return false,
            },
        }
    }
    subject_tokens.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::covers;

    #[test]
    fn a_trailing_wildcard_covers_every_deeper_subject() {
        assert!(covers(
            "integration.evt.>",
            "integration.evt.identity.user.created.v1"
        ));
        assert!(!covers("integration.evt.>", "integration.evt"));
        assert!(!covers(
            "integration.evt.>",
            "integration.cmd.notifier.notification.deliver.v1"
        ));
    }

    #[test]
    fn a_token_wildcard_covers_exactly_one_token() {
        assert!(covers(
            "integration.evt.*.user.created.v1",
            "integration.evt.identity.user.created.v1"
        ));
        assert!(!covers(
            "integration.evt.*.user.created.v1",
            "integration.evt.identity.group.created.v1"
        ));
    }

    #[test]
    fn a_literal_binding_covers_only_itself() {
        assert!(covers(
            "integration.evt.identity.user.created.v1",
            "integration.evt.identity.user.created.v1"
        ));
        assert!(!covers(
            "integration.evt.identity.user.created.v1",
            "integration.evt.identity.user.renamed.v1"
        ));
        assert!(!covers(
            "integration.evt.identity.user.created.v1",
            "integration.evt.identity.user.created.v1.extra"
        ));
    }

    #[test]
    fn a_token_wildcard_never_stands_in_for_several_tokens() {
        assert!(!covers("integration.evt.*", "integration.evt.a.b"));
        assert!(covers("integration.evt.*", "integration.evt.a"));
    }

    #[test]
    fn an_inner_gt_is_a_literal_token_and_covers_nothing() {
        assert!(!covers("integration.>.user", "integration.evt.user"));
        assert!(!covers(
            "integration.>.user",
            "integration.evt.identity.user"
        ));
    }

    #[test]
    fn an_empty_pattern_or_subject_covers_nothing() {
        assert!(!covers("", ""));
        assert!(!covers("", "integration.evt.identity.user.created.v1"));
        assert!(!covers("integration.evt.>", ""));
    }

    #[test]
    fn the_withheld_placeholder_covers_no_real_coordinate() {
        assert!(!covers(
            "integration.evt.__withheld__.>",
            "integration.evt.identity.user.created.v1"
        ));
        assert!(!covers(
            "integration.cmd.__withheld__.>",
            "integration.cmd.notifier.notification.deliver.v1"
        ));
    }
}
