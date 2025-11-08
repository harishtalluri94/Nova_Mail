use crate::jmap::*;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JMAP Session resource
/// Spec: https://jmap.io/spec-core.html#the-jmap-session-resource
pub async fn session(State(_state): State<AppState>) -> impl IntoResponse {
    let session = JmapSession {
        capabilities: Capabilities {
            core: CoreCapability {
                max_size_upload: 50_000_000,        // 50 MB
                max_concurrent_upload: 4,
                max_size_request: 10_000_000,       // 10 MB
                max_concurrent_requests: 4,
                max_calls_in_request: 16,
                max_objects_in_get: 500,
                max_objects_in_set: 500,
                collation_algorithms: vec!["i;ascii-casemap".to_string()],
            },
            mail: MailCapability {
                max_mailboxes_per_email: Some(100),
                max_mailbox_depth: Some(10),
                max_size_mailbox_name: 200,
                max_size_attachments_per_email: 50_000_000,
                email_query_sort_options: vec![
                    "receivedAt".to_string(),
                    "from".to_string(),
                    "to".to_string(),
                    "subject".to_string(),
                ],
                may_create_top_level_mailbox: true,
            },
        },
        accounts: vec![],
        primary_accounts: PrimaryAccounts {
            mail: "default".to_string(),
        },
        username: "user@example.com".to_string(),
        api_url: "/jmap".to_string(),
        download_url: "/jmap/download/{blob_id}".to_string(),
        upload_url: "/jmap/upload/{account_id}".to_string(),
        event_source_url: "/jmap/eventsource".to_string(),
        state: Uuid::new_v4().to_string(),
    };

    Json(session)
}

/// Main JMAP request handler
/// Spec: https://jmap.io/spec-core.html#the-request-object
pub async fn jmap_request(
    State(state): State<AppState>,
    Json(request): Json<JmapRequest>,
) -> Result<impl IntoResponse, JmapError> {
    tracing::debug!(
        "JMAP request with {} method calls",
        request.method_calls.len()
    );

    let mut responses = Vec::new();

    for method_call in request.method_calls {
        let response = match method_call.method.as_str() {
            "Email/query" => handle_email_query(&state, method_call).await?,
            "Email/get" => handle_email_get(&state, method_call).await?,
            "Email/set" => handle_email_set(&state, method_call).await?,
            "Mailbox/get" => handle_mailbox_get(&state, method_call).await?,
            "Mailbox/query" => handle_mailbox_query(&state, method_call).await?,
            "Mailbox/set" => handle_mailbox_set(&state, method_call).await?,
            _ => {
                return Err(JmapError::UnknownMethod(method_call.method.clone()));
            }
        };

        responses.push(response);
    }

    Ok(Json(JmapResponse {
        method_responses: responses,
        session_state: request.using.get("urn:ietf:params:jmap:core").cloned(),
    }))
}

async fn handle_email_query(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Email/query
    // Query database for emails matching filters
    // Return list of email IDs

    Ok(MethodResponse {
        method: "Email/query".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "queryState": Uuid::new_v4().to_string(),
            "canCalculateChanges": false,
            "position": 0,
            "ids": [],
            "total": 0,
            "limit": 50
        }),
    })
}

async fn handle_email_get(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Email/get
    // Fetch email details from database and object storage

    Ok(MethodResponse {
        method: "Email/get".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "state": Uuid::new_v4().to_string(),
            "list": [],
            "notFound": []
        }),
    })
}

async fn handle_email_set(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Email/set
    // Create, update, or destroy emails

    Ok(MethodResponse {
        method: "Email/set".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "oldState": Uuid::new_v4().to_string(),
            "newState": Uuid::new_v4().to_string(),
            "created": {},
            "updated": {},
            "destroyed": []
        }),
    })
}

async fn handle_mailbox_get(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Mailbox/get

    Ok(MethodResponse {
        method: "Mailbox/get".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "state": Uuid::new_v4().to_string(),
            "list": [],
            "notFound": []
        }),
    })
}

async fn handle_mailbox_query(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Mailbox/query

    Ok(MethodResponse {
        method: "Mailbox/query".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "queryState": Uuid::new_v4().to_string(),
            "canCalculateChanges": false,
            "position": 0,
            "ids": [],
            "total": 0
        }),
    })
}

async fn handle_mailbox_set(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Mailbox/set

    Ok(MethodResponse {
        method: "Mailbox/set".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "oldState": Uuid::new_v4().to_string(),
            "newState": Uuid::new_v4().to_string(),
            "created": {},
            "updated": {},
            "destroyed": []
        }),
    })
}

/// JMAP Upload endpoint
pub async fn upload(
    State(_state): State<AppState>,
    Path(_account_id): Path<String>,
    _body: bytes::Bytes,
) -> Result<impl IntoResponse, JmapError> {
    // TODO: Implement blob upload
    // Store in object storage, return blob ID

    Ok(Json(serde_json::json!({
        "accountId": "default",
        "blobId": Uuid::new_v4().to_string(),
        "type": "application/octet-stream",
        "size": 0
    })))
}

/// JMAP Download endpoint
pub async fn download(
    State(_state): State<AppState>,
    Path(_blob_id): Path<String>,
) -> Result<impl IntoResponse, JmapError> {
    // TODO: Implement blob download
    // Fetch from object storage

    Ok((StatusCode::NOT_FOUND, "Blob not found"))
}

#[derive(Debug, thiserror::Error)]
pub enum JmapError {
    #[error("Unknown method: {0}")]
    UnknownMethod(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for JmapError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_type, description) = match self {
            JmapError::UnknownMethod(method) => (
                StatusCode::BAD_REQUEST,
                "unknownMethod",
                format!("Unknown method: {}", method),
            ),
            JmapError::InvalidArguments(msg) => (
                StatusCode::BAD_REQUEST,
                "invalidArguments",
                msg,
            ),
            JmapError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "serverFail",
                "Internal server error".to_string(),
            ),
            JmapError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "notFound",
                msg,
            ),
        };

        (
            status,
            Json(serde_json::json!({
                "type": error_type,
                "description": description
            })),
        )
            .into_response()
    }
}
