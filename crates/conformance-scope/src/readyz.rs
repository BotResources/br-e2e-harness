use std::time::Duration;

use reqwest::StatusCode;

use crate::error::{ConformanceError, Result};

pub struct ReadyzProbe {
    url: String,
    client: reqwest::Client,
}

impl ReadyzProbe {
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        require_http_url(&url)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| ConformanceError::Readyz(format!("building http client: {e}")))?;
        Ok(Self { url, client })
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

    pub async fn body(&self) -> Option<String> {
        let resp = self.client.get(&self.url).send().await.ok()?;
        resp.text().await.ok()
    }

    pub async fn is_ready(&self) -> bool {
        self.status().await == Some(StatusCode::OK)
    }

    pub async fn is_not_ready(&self) -> bool {
        self.status().await == Some(StatusCode::SERVICE_UNAVAILABLE)
    }
}

fn require_http_url(raw: &str) -> Result<()> {
    let invalid = |reason: &str| {
        ConformanceError::InvalidInput(format!("/readyz URL `{raw}` is not usable: {reason}"))
    };
    let parsed = reqwest::Url::parse(raw).map_err(|e| invalid(&e.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(invalid("the scheme must be http or https"));
    }
    if parsed.host_str().is_none() {
        return Err(invalid("no host"));
    }
    Ok(())
}
