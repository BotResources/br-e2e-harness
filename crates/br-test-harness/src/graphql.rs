use br_core_auth::{Passport, PassportHeader};
use reqwest::StatusCode;
use serde_json::Value;

pub struct GraphqlClient {
    base_url: String,
    client: reqwest::Client,
    path: String,
}

impl GraphqlClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
            path: "/graphql".to_string(),
        }
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }

    pub async fn query(&self, passport: &Passport, query: &str, vars: Value) -> Value {
        self.client
            .post(format!("{}{}", self.base_url, self.path))
            .header("X-Passport", passport.to_header())
            .json(&serde_json::json!({ "query": query, "variables": vars }))
            .send()
            .await
            .expect("graphql request failed")
            .json()
            .await
            .expect("graphql response not JSON")
    }

    pub async fn query_unauthenticated(&self, query: &str, vars: Value) -> (StatusCode, Value) {
        let resp = self
            .client
            .post(format!("{}{}", self.base_url, self.path))
            .json(&serde_json::json!({ "query": query, "variables": vars }))
            .send()
            .await
            .expect("graphql request failed");
        let status = resp.status();
        (status, body_json_or_text(resp).await)
    }

    pub async fn query_with_passport_header(
        &self,
        passport_header: &str,
        query: &str,
        vars: Value,
    ) -> (StatusCode, Value) {
        let resp = self
            .client
            .post(format!("{}{}", self.base_url, self.path))
            .header("X-Passport", passport_header)
            .json(&serde_json::json!({ "query": query, "variables": vars }))
            .send()
            .await
            .expect("graphql request failed");
        let status = resp.status();
        (status, body_json_or_text(resp).await)
    }

    pub async fn post_raw(
        &self,
        path: &str,
        headers: &[(&str, &str)],
        body: Value,
    ) -> (StatusCode, Value) {
        let mut req = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .json(&body);
        for (key, value) in headers {
            req = req.header(*key, *value);
        }
        let resp = req.send().await.expect("request failed");
        let status = resp.status();
        (status, body_json_or_text(resp).await)
    }

    pub async fn get_raw(&self, path: &str) -> (StatusCode, String) {
        let resp = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .expect("request failed");
        let status = resp.status();
        (status, resp.text().await.unwrap_or_default())
    }
}

async fn body_json_or_text(resp: reqwest::Response) -> Value {
    let text = resp.text().await.unwrap_or_default();
    serde_json::from_str(&text).unwrap_or(Value::String(text))
}
