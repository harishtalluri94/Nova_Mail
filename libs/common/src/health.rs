use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: ServiceStatus,
    pub version: String,
    pub checks: HashMap<String, CheckResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub status: ServiceStatus,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

impl CheckResult {
    pub fn healthy() -> Self {
        Self {
            status: ServiceStatus::Healthy,
            message: None,
            latency_ms: None,
        }
    }

    pub fn degraded(message: String) -> Self {
        Self {
            status: ServiceStatus::Degraded,
            message: Some(message),
            latency_ms: None,
        }
    }

    pub fn unhealthy(message: String) -> Self {
        Self {
            status: ServiceStatus::Unhealthy,
            message: Some(message),
            latency_ms: None,
        }
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }
}

impl HealthStatus {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Healthy,
            version: version.into(),
            checks: HashMap::new(),
        }
    }

    pub fn add_check(&mut self, name: impl Into<String>, result: CheckResult) {
        // Update overall status based on check results
        match result.status {
            ServiceStatus::Unhealthy => self.status = ServiceStatus::Unhealthy,
            ServiceStatus::Degraded if self.status == ServiceStatus::Healthy => {
                self.status = ServiceStatus::Degraded
            }
            _ => {}
        }
        self.checks.insert(name.into(), result);
    }

    pub fn is_healthy(&self) -> bool {
        self.status == ServiceStatus::Healthy
    }
}

impl IntoResponse for HealthStatus {
    fn into_response(self) -> axum::response::Response {
        let status_code = match self.status {
            ServiceStatus::Healthy => StatusCode::OK,
            ServiceStatus::Degraded => StatusCode::OK, // Still accepting traffic
            ServiceStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        };

        (status_code, Json(self)).into_response()
    }
}

/// Simple liveness probe - just returns 200 OK if service is running
pub async fn liveness_handler() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness probe - checks dependencies
pub async fn readiness_handler(health: HealthStatus) -> impl IntoResponse {
    health
}
