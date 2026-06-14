use std::time::Duration;

use br_core_auth::{Passport, PassportHeader};
use reqwest::StatusCode;

use crate::error::{ConformanceError, Result};

pub const PASSPORT_PATH: &str = "/internal/passport";
pub const PASSPORT_HEADER: &str = "X-Passport";

#[derive(Debug, Clone)]
pub enum Resolution {
    Resolved(Passport),
    Anonymous,
}

impl Resolution {
    pub fn label(&self) -> String {
        match self {
            Resolution::Anonymous => "anonymous(no X-Passport)".to_string(),
            Resolution::Resolved(passport) => match passport {
                Passport::Human { user_id, .. } => format!("human(user_id={user_id})"),
                Passport::Service {
                    service_account_id, ..
                } => format!("service(service_account_id={service_account_id})"),
            },
        }
    }
}

pub struct PassportEndpoint {
    base_url: String,
    client: reqwest::Client,
}

impl PassportEndpoint {
    pub fn new(base_url: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| ConformanceError::Request(format!("building http client: {e}")))?;
        Ok(Self {
            base_url: base_url.to_string(),
            client,
        })
    }

    pub async fn resolve_bearer(&self, raw_token: &str) -> Result<Resolution> {
        self.resolve(Some(&format!("Bearer {raw_token}"))).await
    }

    pub async fn resolve_anonymous(&self) -> Result<Resolution> {
        self.resolve(None).await
    }

    pub async fn resolve(&self, authorization: Option<&str>) -> Result<Resolution> {
        let url = format!("{}{PASSPORT_PATH}", self.base_url);
        let mut request = self.client.get(&url);
        if let Some(authorization) = authorization {
            request = request.header("Authorization", authorization);
        }
        let response = request
            .send()
            .await
            .map_err(|e| ConformanceError::Request(format!("GET {url}: {e}")))?;

        if response.status() != StatusCode::OK {
            return Err(ConformanceError::Request(format!(
                "GET {url} returned {}, expected 200",
                response.status()
            )));
        }

        let header = match response.headers().get(PASSPORT_HEADER) {
            None => return Ok(Resolution::Anonymous),
            Some(value) => value.to_str().map_err(|e| {
                ConformanceError::NonConformantPassport(format!("X-Passport not valid ascii: {e}"))
            })?,
        };

        let passport = Passport::from_header(header)
            .map_err(|e| ConformanceError::NonConformantPassport(format!("{e}")))?;
        Ok(Resolution::Resolved(passport))
    }
}
