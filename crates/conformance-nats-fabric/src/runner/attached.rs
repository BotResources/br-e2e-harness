use br_core_directory::PublishedUser;
use br_test_harness::{FabricTestNats, FixedStream};
use br_util_nats_fabric::{
    INTEGRATION_CMD, INTEGRATION_EVT, KV_PUBLISHED_LANGUAGE, PublishedLanguagePublisher,
    PublishedLanguageReader,
};

use super::execute::{Opened, Setup};
use crate::harness::namespaced_prefix;

pub(super) async fn require_topology(harness: &FabricTestNats, setup: Setup) -> Result<(), String> {
    match setup {
        Setup::FixedStreams | Setup::WidenedDurable => {
            if harness.fixed_streams_present().await {
                Ok(())
            } else {
                Err(format!(
                    "the fixed streams {INTEGRATION_CMD} and {INTEGRATION_EVT} must exist on the \
                     target: the platform declares them out of band and this tool never creates them"
                ))
            }
        }
        Setup::PublishedLanguage => {
            if harness.published_language_present().await {
                Ok(())
            } else {
                Err(format!(
                    "the {KV_PUBLISHED_LANGUAGE} bucket must exist on the target: the platform \
                     declares it out of band and this tool never creates it"
                ))
            }
        }
    }
}

pub(super) async fn clean_attached(opened: &Opened, setup: Setup) -> Result<(), String> {
    match setup {
        Setup::FixedStreams => Ok(()),
        Setup::WidenedDurable => {
            let marker = opened
                .widened
                .as_ref()
                .expect("the widened durable was set up");
            opened
                .harness
                .delete_durable(FixedStream::Evt, &marker.durable)
                .await;
            Ok(())
        }
        Setup::PublishedLanguage => retract_run_keys(&opened.harness).await,
    }
}

async fn retract_run_keys(harness: &FabricTestNats) -> Result<(), String> {
    let fabric = harness.fabric_owned();
    let prefix = namespaced_prefix(harness, "");
    let leftovers = |what: &str, e: crate::error::ConformanceError| {
        format!("could not remove the {what} this run left in {KV_PUBLISHED_LANGUAGE}: {e}")
    };
    let reader: PublishedLanguageReader<PublishedUser> = PublishedLanguageReader::open(&fabric)
        .await
        .map_err(|e| leftovers("keys", e.into()))?;
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&fabric)
            .await
            .map_err(|e| leftovers("keys", e.into()))?;
    let keys = reader
        .keys(&prefix)
        .await
        .map_err(|e| leftovers("keys", e.into()))?;
    for key in keys {
        publisher
            .retract(&key)
            .await
            .map_err(|e| leftovers(key.as_str(), e.into()))?;
    }
    Ok(())
}
