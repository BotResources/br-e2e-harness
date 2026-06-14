use std::time::Duration;

use reqwest::StatusCode;

use crate::error::{ConformanceError, Result};

pub struct ReadyzProbe {
    url: String,
    client: reqwest::Client,
}

impl ReadyzProbe {
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| ConformanceError::Readyz(format!("building http client: {e}")))?;
        Ok(Self {
            url: url.into(),
            client,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn status(&self) -> Option<StatusCode> {
        self.client
            .get(&self.url)
            .send()
            .await
            .ok()
            .map(|resp| resp.status())
    }

    pub async fn is_ready(&self) -> bool {
        self.status().await == Some(StatusCode::OK)
    }

    pub async fn is_not_ready(&self) -> bool {
        self.status().await == Some(StatusCode::SERVICE_UNAVAILABLE)
    }
}
