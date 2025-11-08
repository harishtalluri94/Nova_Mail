use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RateLimitCheckRequest {
    pub user_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub action: String, // "send_email", "signup", "login", etc.
}

#[derive(Debug, Serialize)]
pub struct RateLimitCheckResponse {
    pub allowed: bool,
    pub limit: u64,
    pub remaining: u64,
    pub reset_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct ReputationCheckRequest {
    pub ip_address: Option<String>,
    pub domain: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReputationCheckResponse {
    pub score: f32, // 0.0 (bad) to 1.0 (good)
    pub blocklisted: bool,
    pub allowlisted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnomalyCheckRequest {
    pub user_id: Uuid,
    pub event_type: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AnomalyCheckResponse {
    pub is_anomalous: bool,
    pub confidence: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlocklistRequest {
    pub identifier: String, // IP, domain, or email
    pub identifier_type: BlocklistType,
    pub reason: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlocklistType {
    Ip,
    Domain,
    Email,
}

#[derive(Debug, Deserialize)]
pub struct AbuseReportRequest {
    pub reporter_email: String,
    pub reported_content: String,
    pub abuse_type: String,
    pub description: String,
}

/// Check if a request should be rate limited
pub async fn check_rate_limit(
    State(state): State<AppState>,
    Json(request): Json<RateLimitCheckRequest>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::debug!("Rate limit check: action={}", request.action);

    let result = state
        .rate_limiter
        .check(&request.user_id, &request.ip_address, &request.action)
        .await?;

    Ok(Json(RateLimitCheckResponse {
        allowed: result.allowed,
        limit: result.limit,
        remaining: result.remaining,
        reset_at: result.reset_at,
    }))
}

/// Check reputation of an IP, domain, or email
pub async fn check_reputation(
    State(state): State<AppState>,
    Json(request): Json<ReputationCheckRequest>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::debug!("Reputation check: {:?}", request);

    let result = state
        .reputation
        .check(&request.ip_address, &request.domain, &request.email)
        .await?;

    Ok(Json(ReputationCheckResponse {
        score: result.score,
        blocklisted: result.blocklisted,
        allowlisted: result.allowlisted,
        reasons: result.reasons,
    }))
}

/// Check for anomalous behavior
pub async fn check_anomaly(
    State(state): State<AppState>,
    Json(request): Json<AnomalyCheckRequest>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::debug!("Anomaly check: user_id={}, event={}", request.user_id, request.event_type);

    let result = state
        .anomaly
        .detect(&request.user_id, &request.event_type, &request.metadata)
        .await?;

    Ok(Json(AnomalyCheckResponse {
        is_anomalous: result.is_anomalous,
        confidence: result.confidence,
        reasons: result.reasons,
    }))
}

/// Add an identifier to the blocklist
pub async fn add_to_blocklist(
    State(state): State<AppState>,
    Json(request): Json<BlocklistRequest>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::info!("Adding to blocklist: {} ({:?})", request.identifier, request.identifier_type);

    state
        .reputation
        .add_to_blocklist(
            &request.identifier,
            &request.identifier_type,
            &request.reason,
            request.expires_at,
        )
        .await?;

    Ok(StatusCode::CREATED)
}

/// Remove an identifier from the blocklist
pub async fn remove_from_blocklist(
    State(state): State<AppState>,
    Path(identifier): Path<String>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::info!("Removing from blocklist: {}", identifier);

    state.reputation.remove_from_blocklist(&identifier).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Add an identifier to the allowlist
pub async fn add_to_allowlist(
    State(state): State<AppState>,
    Json(request): Json<BlocklistRequest>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::info!("Adding to allowlist: {} ({:?})", request.identifier, request.identifier_type);

    state
        .reputation
        .add_to_allowlist(
            &request.identifier,
            &request.identifier_type,
            &request.reason,
        )
        .await?;

    Ok(StatusCode::CREATED)
}

/// Remove an identifier from the allowlist
pub async fn remove_from_allowlist(
    State(state): State<AppState>,
    Path(identifier): Path<String>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::info!("Removing from allowlist: {}", identifier);

    state.reputation.remove_from_allowlist(&identifier).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Report abuse
pub async fn report_abuse(
    State(state): State<AppState>,
    Json(request): Json<AbuseReportRequest>,
) -> Result<impl IntoResponse, AntiAbuseError> {
    tracing::info!("Abuse report from: {}, type: {}", request.reporter_email, request.abuse_type);

    // Store abuse report in database
    sqlx::query!(
        r#"
        INSERT INTO abuse_events (
            id, event_type, user_id, ip_address, details, created_at
        ) VALUES ($1, $2, NULL, NULL, $3, NOW())
        "#,
        Uuid::new_v4(),
        request.abuse_type,
        serde_json::json!({
            "reporter": request.reporter_email,
            "content": request.reported_content,
            "description": request.description
        })
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}

#[derive(Debug, thiserror::Error)]
pub enum AntiAbuseError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}

impl IntoResponse for AntiAbuseError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AntiAbuseError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            ),
            AntiAbuseError::RedisError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cache error".to_string(),
            ),
            AntiAbuseError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AntiAbuseError::InternalError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error".to_string(),
            ),
        };

        (
            status,
            Json(serde_json::json!({
                "error": message
            })),
        )
            .into_response()
    }
}
