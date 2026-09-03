use std::time::Duration;

use async_nats::jetstream::consumer;

pub const LIB_ACK_WAIT: Duration = Duration::from_secs(30);
pub const LIB_MAX_ACK_PENDING: i64 = 256;
pub const HARNESS_ACK_WAIT: Duration = Duration::from_secs(2);
pub const SERVER_DEFAULT_MAX_ACK_PENDING: i64 = 0;
const UNLIMITED_MAX_DELIVER: i64 = -1;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurableConfig {
    pub ack_wait: Duration,
    pub max_deliver: Option<i64>,
    pub max_ack_pending: i64,
}

impl Default for DurableConfig {
    fn default() -> Self {
        Self {
            ack_wait: LIB_ACK_WAIT,
            max_deliver: None,
            max_ack_pending: LIB_MAX_ACK_PENDING,
        }
    }
}

impl DurableConfig {
    pub fn harness() -> Self {
        Self {
            ack_wait: HARNESS_ACK_WAIT,
            max_deliver: None,
            max_ack_pending: SERVER_DEFAULT_MAX_ACK_PENDING,
        }
    }

    pub fn ack_wait(mut self, ack_wait: Duration) -> Self {
        self.ack_wait = ack_wait;
        self
    }

    pub fn max_deliver(mut self, max_deliver: i64) -> Self {
        self.max_deliver = Some(max_deliver);
        self
    }

    pub fn unlimited_deliver(mut self) -> Self {
        self.max_deliver = None;
        self
    }

    pub fn max_ack_pending(mut self, max_ack_pending: i64) -> Self {
        self.max_ack_pending = max_ack_pending;
        self
    }
}

pub(super) fn pull_config(
    durable: &str,
    filter: &str,
    config: &DurableConfig,
) -> consumer::pull::Config {
    consumer::pull::Config {
        durable_name: Some(durable.to_string()),
        filter_subjects: vec![filter.to_string()],
        ack_policy: consumer::AckPolicy::Explicit,
        deliver_policy: consumer::DeliverPolicy::All,
        replay_policy: consumer::ReplayPolicy::Instant,
        ack_wait: config.ack_wait,
        max_deliver: config.max_deliver.unwrap_or(UNLIMITED_MAX_DELIVER),
        max_ack_pending: config.max_ack_pending,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_mirrors_the_lib_consumer_tuning() {
        let config = DurableConfig::default();
        assert_eq!(config.ack_wait, Duration::from_secs(30));
        assert_eq!(config.max_ack_pending, 256);
        assert_eq!(config.max_deliver, None);
    }

    #[test]
    fn the_harness_constructor_keeps_the_shipped_provisioner_values() {
        let config = DurableConfig::harness();
        assert_eq!(config.ack_wait, Duration::from_secs(2));
        assert_eq!(config.max_ack_pending, 0);
        assert_eq!(config.max_deliver, None);
    }

    #[test]
    fn an_absent_max_deliver_renders_unlimited() {
        let rendered = pull_config("d", "integration.cmd.a.b.c.v1", &DurableConfig::harness());
        assert_eq!(rendered.max_deliver, -1);
    }

    #[test]
    fn a_finite_budget_threads_through_leaving_the_frozen_contract_fixed() {
        let config = DurableConfig::harness()
            .max_deliver(3)
            .ack_wait(Duration::from_millis(750))
            .max_ack_pending(4);
        let rendered = pull_config("d", "integration.cmd.a.b.c.v1", &config);
        assert_eq!(rendered.max_deliver, 3);
        assert_eq!(rendered.ack_wait, Duration::from_millis(750));
        assert_eq!(rendered.max_ack_pending, 4);
        assert_eq!(rendered.filter_subjects, vec!["integration.cmd.a.b.c.v1"]);
        assert!(matches!(rendered.ack_policy, consumer::AckPolicy::Explicit));
        assert!(matches!(
            rendered.deliver_policy,
            consumer::DeliverPolicy::All
        ));
        assert!(matches!(
            rendered.replay_policy,
            consumer::ReplayPolicy::Instant
        ));
    }

    #[test]
    fn unlimited_deliver_clears_a_finite_budget() {
        let config = DurableConfig::default().max_deliver(2).unlimited_deliver();
        assert_eq!(config.max_deliver, None);
    }
}
