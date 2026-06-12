use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt as _;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use oidc_test_idp::{IdpConfig, IdpState, router};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const ISSUER: &str = "http://idp.test";
const CLIENT_ID: &str = "e2e-client";

fn test_app() -> Router {
    router(Arc::new(IdpState::new(IdpConfig {
        issuer: format!("{ISSUER}/"),
        key_pool_size: 3,
        initial_published: 2,
        default_client_id: CLIENT_ID.to_string(),
    })))
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn post(app: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn decoding_key_for(jwks: &Value, kid: &str) -> DecodingKey {
    let set: JwkSet = serde_json::from_value(jwks.clone()).expect("JWKS must parse as a JwkSet");
    let jwk = set.find(kid).expect("kid must be in the JWKS");
    DecodingKey::from_jwk(jwk).expect("JWK must convert to a decoding key")
}

fn strict_validation() -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[CLIENT_ID]);
    validation.leeway = 0;
    validation
}

#[tokio::test]
async fn discovery_serves_issuer_and_jwks_uri_with_trailing_slash_trimmed() {
    let app = test_app();
    let (status, doc) = get(&app, "/.well-known/openid-configuration").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(doc["issuer"], ISSUER);
    assert_eq!(doc["jwks_uri"], format!("{ISSUER}/jwks"));
}

#[tokio::test]
async fn jwks_serves_only_the_published_keys() {
    let app = test_app();
    let (status, jwks) = get(&app, "/jwks").await;
    assert_eq!(status, StatusCode::OK);
    let kids: Vec<&str> = jwks["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["kid"].as_str().unwrap())
        .collect();
    assert_eq!(kids, ["e2e-key-0", "e2e-key-1"]);
}

#[tokio::test]
async fn minted_token_verifies_against_the_jwks() {
    let app = test_app();
    let (status, minted) = post(&app, "/admin/mint", json!({"email": "alice@example.com"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(minted["kid"], "e2e-key-0");

    let (_, jwks) = get(&app, "/jwks").await;
    let key = decoding_key_for(&jwks, "e2e-key-0");
    let token = jsonwebtoken::decode::<Value>(
        minted["id_token"].as_str().unwrap(),
        &key,
        &strict_validation(),
    )
    .expect("token minted with the active key must verify");
    assert_eq!(token.claims["email"], "alice@example.com");
    assert_eq!(token.claims["sub"], "alice@example.com");
}

#[tokio::test]
async fn email_claim_name_is_pilotable() {
    let app = test_app();
    let (_, minted) = post(
        &app,
        "/admin/mint",
        json!({"email": "bob@example.com", "email_claim": "preferred_username"}),
    )
    .await;
    let (_, jwks) = get(&app, "/jwks").await;
    let key = decoding_key_for(&jwks, "e2e-key-0");
    let token = jsonwebtoken::decode::<Value>(
        minted["id_token"].as_str().unwrap(),
        &key,
        &strict_validation(),
    )
    .unwrap();
    assert_eq!(token.claims["preferred_username"], "bob@example.com");
    assert!(token.claims.get("email").is_none());
}

#[tokio::test]
async fn unpublished_kid_signs_but_never_appears_in_the_jwks() {
    let app = test_app();
    let (status, minted) = post(
        &app,
        "/admin/mint",
        json!({"email": "eve@example.com", "kid": "e2e-key-2"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(minted["kid"], "e2e-key-2");

    let (_, jwks) = get(&app, "/jwks").await;
    let set: JwkSet = serde_json::from_value(jwks).unwrap();
    assert!(
        set.find("e2e-key-2").is_none(),
        "an unpublished key must not be vouched for by the JWKS"
    );
}

#[tokio::test]
async fn expired_token_fails_verification_with_expired_signature() {
    let app = test_app();
    let (_, minted) = post(
        &app,
        "/admin/mint",
        json!({"email": "late@example.com", "expires_in_secs": -60}),
    )
    .await;
    let (_, jwks) = get(&app, "/jwks").await;
    let key = decoding_key_for(&jwks, "e2e-key-0");
    let error = jsonwebtoken::decode::<Value>(
        minted["id_token"].as_str().unwrap(),
        &key,
        &strict_validation(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &jsonwebtoken::errors::ErrorKind::ExpiredSignature
    );
}

#[tokio::test]
async fn extra_claims_override_generated_ones() {
    let app = test_app();
    let (_, minted) = post(
        &app,
        "/admin/mint",
        json!({"email": "mallory@example.com", "claims": {"iss": "http://somewhere-else"}}),
    )
    .await;
    let (_, jwks) = get(&app, "/jwks").await;
    let key = decoding_key_for(&jwks, "e2e-key-0");
    let error = jsonwebtoken::decode::<Value>(
        minted["id_token"].as_str().unwrap(),
        &key,
        &strict_validation(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &jsonwebtoken::errors::ErrorKind::InvalidIssuer
    );
}

#[tokio::test]
async fn omitting_the_kid_header_is_pilotable() {
    let app = test_app();
    let (_, minted) = post(
        &app,
        "/admin/mint",
        json!({"email": "nokid@example.com", "omit_kid_header": true}),
    )
    .await;
    let header = jsonwebtoken::decode_header(minted["id_token"].as_str().unwrap()).unwrap();
    assert!(header.kid.is_none());
    assert_eq!(
        minted["kid"], "e2e-key-0",
        "the response reports the signing key even when the JWT header omits it"
    );
}

#[tokio::test]
async fn explicit_publish_adds_a_previously_unpublished_kid_to_the_jwks() {
    let app = test_app();
    let (status, state) = post(&app, "/admin/rotate", json!({"publish": ["e2e-key-2"]})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        state["published_kids"],
        json!(["e2e-key-0", "e2e-key-1", "e2e-key-2"])
    );
    assert_eq!(
        state["active_kid"], "e2e-key-0",
        "publish alone must not change the active signing key"
    );

    let (_, jwks) = get(&app, "/jwks").await;
    let set: JwkSet = serde_json::from_value(jwks).unwrap();
    assert!(set.find("e2e-key-2").is_some());
}

#[tokio::test]
async fn default_rotate_publishes_the_next_key_and_makes_it_active() {
    let app = test_app();
    let (status, state) = post(&app, "/admin/rotate", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["active_kid"], "e2e-key-2");
    assert_eq!(
        state["published_kids"],
        json!(["e2e-key-0", "e2e-key-1", "e2e-key-2"])
    );

    let (_, minted) = post(&app, "/admin/mint", json!({"email": "rotated@example.com"})).await;
    assert_eq!(minted["kid"], "e2e-key-2");

    let (status, error) = post(&app, "/admin/rotate", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "pool exhausted: {error}");
}

#[tokio::test]
async fn explicit_rotate_can_unpublish_and_switch_active() {
    let app = test_app();
    let (status, state) = post(
        &app,
        "/admin/rotate",
        json!({"unpublish": ["e2e-key-0"], "active": "e2e-key-1"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["active_kid"], "e2e-key-1");
    assert_eq!(state["published_kids"], json!(["e2e-key-1"]));
}

#[tokio::test]
async fn unknown_kid_is_a_404() {
    let app = test_app();
    let (status, error) = post(
        &app,
        "/admin/mint",
        json!({"email": "x@example.com", "kid": "no-such-kid"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(error["error"].as_str().unwrap().contains("no-such-kid"));
}

#[tokio::test]
async fn fetch_counters_count_and_reset_restores_everything() {
    let app = test_app();
    get(&app, "/jwks").await;
    get(&app, "/jwks").await;
    get(&app, "/.well-known/openid-configuration").await;
    post(&app, "/admin/rotate", json!({})).await;

    let (_, state) = get(&app, "/admin/state").await;
    assert_eq!(state["jwks_fetches"], 2);
    assert_eq!(state["discovery_fetches"], 1);
    assert_eq!(state["active_kid"], "e2e-key-2");

    let (status, state) = post(&app, "/admin/reset", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["jwks_fetches"], 0);
    assert_eq!(state["discovery_fetches"], 0);
    assert_eq!(state["active_kid"], "e2e-key-0");
    assert_eq!(state["published_kids"], json!(["e2e-key-0", "e2e-key-1"]));
}
