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
pub struct CreateUserRequest {
    pub email: String,
    pub display_name: Option<String>,
    pub storage_quota_gb: Option<i32>,
}

pub async fn list_users(
    State(_state): State<AppState>,
    Path(_tenant_id): Path<Uuid>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"users": []})))
}

pub async fn create_user(
    State(_state): State<AppState>,
    Path(_tenant_id): Path<Uuid>,
    Json(_payload): Json<CreateUserRequest>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(serde_json::json!({"id": Uuid::new_v4()})))
}

pub async fn get_user(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

pub async fn update_user(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(_payload): Json<CreateUserRequest>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

pub async fn delete_user(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
