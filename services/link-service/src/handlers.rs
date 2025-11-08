use crate::signer::SignedToken;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateLinkRequest {
    pub blob_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: i64,
    pub max_downloads: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CreateLinkResponse {
    pub link_id: Uuid,
    pub token: String,
    pub url: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct LinkInfo {
    pub link_id: Uuid,
    pub filename: String,
    pub size_bytes: i64,
    pub download_count: i32,
    pub max_downloads: Option<i32>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub revoked: bool,
}

fn default_ttl_seconds() -> i64 {
    86400 * 7 // 7 days
}

pub async fn create_link(
    State(state): State<AppState>,
    Json(request): Json<CreateLinkRequest>,
) -> Result<impl IntoResponse, LinkError> {
    let link_id = Uuid::new_v4();
    let expires_at =
        chrono::Utc::now() + chrono::Duration::seconds(request.ttl_seconds);

    // Store link metadata in database
    sqlx::query!(
        r#"
        INSERT INTO attachment_links (id, blob_id, filename, content_type, size_bytes, max_downloads, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        link_id,
        request.blob_id,
        request.filename,
        request.content_type,
        request.size_bytes,
        request.max_downloads,
        expires_at
    )
    .execute(&state.db_pool)
    .await
    .map_err(|e| LinkError::DatabaseError(e.to_string()))?;

    // Generate signed token
    let token = state.signer.sign(link_id, expires_at)?;

    // Build URL
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let url = format!("{}/d/{}", base_url, token);

    tracing::info!(
        "Created link {} for blob {} (expires: {})",
        link_id,
        request.blob_id,
        expires_at
    );

    Ok(Json(CreateLinkResponse {
        link_id,
        token,
        url,
        expires_at,
    }))
}

pub async fn get_link_info(
    State(state): State<AppState>,
    Path(link_id): Path<Uuid>,
) -> Result<impl IntoResponse, LinkError> {
    let row = sqlx::query!(
        r#"
        SELECT filename, size_bytes, download_count, max_downloads, expires_at, revoked
        FROM attachment_links
        WHERE id = $1
        "#,
        link_id
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| LinkError::DatabaseError(e.to_string()))?
    .ok_or(LinkError::LinkNotFound)?;

    Ok(Json(LinkInfo {
        link_id,
        filename: row.filename,
        size_bytes: row.size_bytes,
        download_count: row.download_count,
        max_downloads: row.max_downloads,
        expires_at: row.expires_at,
        revoked: row.revoked,
    }))
}

pub async fn revoke_link(
    State(state): State<AppState>,
    Path(link_id): Path<Uuid>,
) -> Result<impl IntoResponse, LinkError> {
    sqlx::query!(
        r#"
        UPDATE attachment_links
        SET revoked = true
        WHERE id = $1
        "#,
        link_id
    )
    .execute(&state.db_pool)
    .await
    .map_err(|e| LinkError::DatabaseError(e.to_string()))?;

    tracing::info!("Revoked link {}", link_id);

    Ok(StatusCode::NO_CONTENT)
}

pub async fn download(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, LinkError> {
    // Verify and decode token
    let signed_token = state.signer.verify(&token)?;

    // Check if expired
    if signed_token.expires_at < chrono::Utc::now() {
        return Err(LinkError::LinkExpired);
    }

    // Get link from database
    let row = sqlx::query!(
        r#"
        SELECT blob_id, filename, content_type, download_count, max_downloads, revoked
        FROM attachment_links
        WHERE id = $1
        "#,
        signed_token.link_id
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| LinkError::DatabaseError(e.to_string()))?
    .ok_or(LinkError::LinkNotFound)?;

    // Check if revoked
    if row.revoked {
        return Err(LinkError::LinkRevoked);
    }

    // Check download limit
    if let Some(max) = row.max_downloads {
        if row.download_count >= max {
            return Err(LinkError::DownloadLimitExceeded);
        }
    }

    // Increment download counter
    sqlx::query!(
        r#"
        UPDATE attachment_links
        SET download_count = download_count + 1
        WHERE id = $1
        "#,
        signed_token.link_id
    )
    .execute(&state.db_pool)
    .await
    .map_err(|e| LinkError::DatabaseError(e.to_string()))?;

    // Fetch blob from object storage
    let blob_data = fetch_blob(&row.blob_id).await?;

    // Build response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        row.content_type.parse().unwrap(),
    );
    headers.insert(
        "Content-Disposition",
        format!("attachment; filename=\"{}\"", row.filename)
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Content-Length",
        blob_data.len().to_string().parse().unwrap(),
    );

    tracing::info!(
        "Download {} (link: {}, downloads: {})",
        row.filename,
        signed_token.link_id,
        row.download_count + 1
    );

    Ok((StatusCode::OK, headers, blob_data))
}

async fn fetch_blob(blob_id: &str) -> Result<Vec<u8>, LinkError> {
    // TODO: Implement actual S3/R2 fetch
    tracing::warn!("Blob fetch not implemented for {}", blob_id);
    Ok(vec![])
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("Link not found")]
    LinkNotFound,

    #[error("Link has expired")]
    LinkExpired,

    #[error("Link has been revoked")]
    LinkRevoked,

    #[error("Download limit exceeded")]
    DownloadLimitExceeded,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl IntoResponse for LinkError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            LinkError::LinkNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            LinkError::LinkExpired => (StatusCode::GONE, self.to_string()),
            LinkError::LinkRevoked => (StatusCode::FORBIDDEN, self.to_string()),
            LinkError::DownloadLimitExceeded => (StatusCode::FORBIDDEN, self.to_string()),
            LinkError::InvalidToken => (StatusCode::BAD_REQUEST, self.to_string()),
            LinkError::DatabaseError(_) | LinkError::InternalError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
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

impl From<anyhow::Error> for LinkError {
    fn from(err: anyhow::Error) -> Self {
        LinkError::InternalError(err.to_string())
    }
}
