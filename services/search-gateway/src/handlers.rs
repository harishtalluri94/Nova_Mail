use crate::query_parser::SearchQuery;
use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub tenant_id: Uuid,
    pub query: String,
    #[serde(default)]
    pub filters: SearchFilters,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Default, Deserialize)]
pub struct SearchFilters {
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub has_attachments: Option<bool>,
    pub date_from: Option<chrono::DateTime<chrono::Utc>>,
    pub date_to: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub message_id: String,
    pub subject: String,
    pub from: String,
    pub received_at: i64,
    pub score: f32,
    pub snippet: Option<String>,
}

fn default_limit() -> usize {
    50
}

pub async fn search(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<impl IntoResponse, SearchError> {
    // Parse and validate query
    let parsed_query = SearchQuery::parse(&request.query, &request.filters)?;

    // Execute search
    let results = state
        .searcher
        .search(request.tenant_id, &parsed_query, request.limit, request.offset)
        .await?;

    let response = SearchResponse {
        total: results.len(),
        offset: request.offset,
        limit: request.limit,
        results: results
            .into_iter()
            .map(|r| SearchResult {
                message_id: r.message_id,
                subject: r.subject,
                from: r.from,
                received_at: r.received_at,
                score: r.score,
                snippet: r.snippet,
            })
            .collect(),
    };

    Ok(Json(response))
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Search error: {0}")]
    SearchError(String),
}

impl IntoResponse for SearchError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            SearchError::InvalidQuery(msg) => (StatusCode::BAD_REQUEST, msg),
            SearchError::SearchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
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
