mod handlers;
mod rate_limiter;
mod reputation;
mod anomaly;

use axum::{
    routing::{get, post},
    Router,
};
use nova_common::{
    config::{load_config, DatabaseConfig, ObservabilityConfig, RedisConfig, ServerConfig},
    db, health, logging, redis_client,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub rate_limiter: Arc<rate_limiter::RateLimiter>,
    pub reputation: Arc<reputation::ReputationService>,
    pub anomaly: Arc<anomaly::AnomalyDetector>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let database_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging("antiabuse", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "antiabuse"));

    tracing::info!("Starting Nova Mail AntiAbuse Service v{}", VERSION);

    let db = db::create_pool(&database_config.url, database_config.max_connections).await?;
    let redis = redis_client::create_client(&redis_config.url).await?;

    // Initialize services
    let rate_limiter = Arc::new(rate_limiter::RateLimiter::new(redis.clone()));
    let reputation = Arc::new(reputation::ReputationService::new(db.clone()));
    let anomaly = Arc::new(anomaly::AnomalyDetector::new(redis.clone()));

    let state = AppState {
        db,
        redis,
        rate_limiter,
        reputation,
        anomaly,
    };

    // Build router
    let app = Router::new()
        .route("/api/v1/check/rate_limit", post(handlers::check_rate_limit))
        .route("/api/v1/check/reputation", post(handlers::check_reputation))
        .route("/api/v1/check/anomaly", post(handlers::check_anomaly))
        .route("/api/v1/blocklist", post(handlers::add_to_blocklist))
        .route("/api/v1/blocklist/:identifier", post(handlers::remove_from_blocklist))
        .route("/api/v1/allowlist", post(handlers::add_to_allowlist))
        .route("/api/v1/allowlist/:identifier", post(handlers::remove_from_allowlist))
        .route("/api/v1/report", post(handlers::report_abuse))
        .route("/healthz", get(health::liveness_handler))
        .route("/readyz", get(health::readiness_handler_with_deps))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("AntiAbuse service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("AntiAbuse service shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
