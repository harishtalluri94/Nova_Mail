use crate::AppState;
use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};

pub async fn stripe_webhook(
    State(_state): State<AppState>,
    _payload: Bytes,
) -> impl IntoResponse {
    // TODO: Implement Stripe webhook handling
    // Verify signature, process events
    StatusCode::OK
}
