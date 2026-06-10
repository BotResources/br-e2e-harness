//! HTTP surface. Public side mimics a real IdP (discovery + JWKS); the
//! `/admin/*` side is the test-only control plane.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::state::{AdminError, IdpState, MintRequest, RotateRequest};

pub fn router(state: Arc<IdpState>) -> Router {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/health", get(health))
        .route("/admin/state", get(admin_state))
        .route("/admin/mint", post(admin_mint))
        .route("/admin/rotate", post(admin_rotate))
        .route("/admin/reset", post(admin_reset))
        .with_state(state)
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let status = match &self {
            AdminError::UnknownKid { .. } => StatusCode::NOT_FOUND,
            AdminError::PoolExhausted(_) => StatusCode::CONFLICT,
            AdminError::Signing(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

async fn discovery(State(state): State<Arc<IdpState>>) -> Json<serde_json::Value> {
    Json(state.discovery_document())
}

async fn jwks(State(state): State<Arc<IdpState>>) -> Json<serde_json::Value> {
    Json(state.jwks_document())
}

async fn health() -> &'static str {
    "ok"
}

async fn admin_state(State(state): State<Arc<IdpState>>) -> Json<serde_json::Value> {
    Json(state.snapshot())
}

async fn admin_mint(
    State(state): State<Arc<IdpState>>,
    Json(req): Json<MintRequest>,
) -> Result<Response, AdminError> {
    let minted = state.mint(&req)?;
    Ok(Json(minted).into_response())
}

async fn admin_rotate(
    State(state): State<Arc<IdpState>>,
    body: Option<Json<RotateRequest>>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let req = body.map(|Json(r)| r).unwrap_or_default();
    Ok(Json(state.rotate(&req)?))
}

async fn admin_reset(State(state): State<Arc<IdpState>>) -> Json<serde_json::Value> {
    Json(state.reset())
}
