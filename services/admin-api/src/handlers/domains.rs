use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDomainRequest {
    pub domain: String,
}

pub async fn list_domains(
    State(_state): State<AppState>,
    Path(_tenant_id): Path<Uuid>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"domains": []})))
}

pub async fn create_domain(
    State(_state): State<AppState>,
    Path(_tenant_id): Path<Uuid>,
    Json(_payload): Json<CreateDomainRequest>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(serde_json::json!({"id": Uuid::new_v4()})))
}

pub async fn get_domain(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

pub async fn rotate_dkim(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"rotated": true})))
}

pub async fn verify_domain(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"verified": false})))
}
