use axum::{http::StatusCode, response::IntoResponse};

/// Initialize metrics (simplified without OpenTelemetry for now)
pub fn init_metrics(_service_name: &str) {
    // Note: OpenTelemetry metrics integration temporarily disabled
    // TODO: Re-enable with correct OpenTelemetry SDK version
    tracing::info!("Metrics initialized (basic mode)");
}

/// Prometheus metrics handler
pub async fn metrics_handler() -> impl IntoResponse {
    // For now, return a basic response
    // TODO: Integrate with actual Prometheus registry
    (
        StatusCode::OK,
        "# HELP nova_mail_up Service up indicator\n# TYPE nova_mail_up gauge\nnova_mail_up 1\n",
    )
        .into_response()
}
