use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use nova_common::health::{CheckResult, HealthStatus};
use serde_json::json;

pub async fn liveness() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let mut health = HealthStatus::new(env!("CARGO_PKG_VERSION"));

    // Check database
    let db_check = match nova_common::db::health_check(&state.db_pool).await {
        Ok(_) => CheckResult::healthy(),
        Err(e) => CheckResult::unhealthy(format!("Database unhealthy: {}", e)),
    };
    health.add_check("database", db_check);

    // Check Redis
    let mut redis_conn = state.redis.clone();
    let redis_check = match nova_common::redis_client::health_check(&mut redis_conn).await {
        Ok(_) => CheckResult::healthy(),
        Err(e) => CheckResult::unhealthy(format!("Redis unhealthy: {}", e)),
    };
    health.add_check("redis", redis_check);

    health
}

pub async fn metrics() -> impl IntoResponse {
    // TODO: Implement Prometheus metrics export
    (StatusCode::OK, "# Metrics endpoint placeholder\n")
}
