use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::Authentication(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            Error::Authorization(_) => (StatusCode::FORBIDDEN, self.to_string()),
            Error::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            Error::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Error::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            Error::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            Error::QuotaExceeded(_) => (StatusCode::INSUFFICIENT_STORAGE, self.to_string()),
            Error::Database(_) | Error::Redis(_) | Error::Internal(_) | Error::Io(_) => {
                tracing::error!("Internal error: {:?}", self);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            Error::Config(_) => {
                tracing::error!("Configuration error: {:?}", self);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Service misconfigured".to_string(),
                )
            }
            Error::Serialization(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Serialization error".to_string(),
            ),
            Error::Other(ref e) => {
                tracing::error!("Unexpected error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
