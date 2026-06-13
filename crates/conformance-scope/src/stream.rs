use async_nats::jetstream::{self, stream};

use crate::error::{ConformanceError, Result};
use crate::subjects::STREAM_SUBJECTS;

pub async fn create_handshake_stream(
    js: &jetstream::Context,
    name: &str,
) -> Result<stream::Stream> {
    js.create_stream(stream::Config {
        name: name.to_string(),
        subjects: vec![STREAM_SUBJECTS.to_string()],
        ..Default::default()
    })
    .await
    .map_err(|e| ConformanceError::Jetstream(format!("create stream '{name}': {e}")))
}
