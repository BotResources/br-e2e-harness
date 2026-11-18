use conformance_scope::{ScopeHarness, Subject, SubjectConfig};

use crate::fixture::{SCOPES, SERVICE_KEY};

pub struct AttachedService {
    harness: ScopeHarness,
    subject: Subject,
    pub nats_url: String,
    pub readyz_url: String,
}

impl AttachedService {
    pub async fn boot() -> Self {
        Self::boot_with(|config| config).await
    }

    pub async fn boot_with(adjust: impl FnOnce(SubjectConfig) -> SubjectConfig) -> Self {
        let harness = ScopeHarness::start().await.expect("harness");
        let nats_url = harness.nats_url();
        let config = adjust(
            SubjectConfig::new(
                &nats_url,
                harness.stream_name(),
                harness.event_stream_name(),
                SERVICE_KEY,
            )
            .scope_keys(SCOPES)
            .label_key("label.notifier")
            .description_key("desc.notifier")
            .wait_timeout("500ms"),
        );
        let subject = Subject::start(harness.binary(), &config)
            .await
            .expect("start the subject");
        let readyz_url = format!("{}/readyz", subject.base_url());
        Self {
            harness,
            subject,
            nats_url,
            readyz_url,
        }
    }

    pub async fn shutdown(self) {
        self.subject.shutdown().await;
        self.harness.shutdown().await;
    }
}
