use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{command_subject, event_subject};

use super::FabricTestNats;
use super::observe::FixedStream;

impl FabricTestNats {
    pub async fn purge_command_stream(&self) -> u64 {
        self.purge(FixedStream::Cmd, None).await
    }

    pub async fn purge_event_stream(&self) -> u64 {
        self.purge(FixedStream::Evt, None).await
    }

    pub async fn purge_command_subject(&self, coords: &CommandCoords) -> u64 {
        self.purge(FixedStream::Cmd, Some(command_subject(coords)))
            .await
    }

    pub async fn purge_event_subject(&self, coords: &EventCoords) -> u64 {
        self.purge(FixedStream::Evt, Some(event_subject(coords)))
            .await
    }

    async fn purge(&self, stream: FixedStream, subject: Option<String>) -> u64 {
        let name = stream.name();
        let js_stream = self
            .js
            .get_stream(name)
            .await
            .unwrap_or_else(|e| panic!("get fixed stream {name} to purge it: {e}"));
        let response = match &subject {
            Some(subject) => js_stream.purge().filter(subject.as_str()).await,
            None => js_stream.purge().await,
        };
        let purged = response.unwrap_or_else(|e| {
            let scope = subject.as_deref().unwrap_or("<whole stream>");
            panic!("purge {name} filtered on {scope}: {e}")
        });
        purged.purged
    }
}
