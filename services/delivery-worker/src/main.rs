mod worker;

use axum::{routing::get, Router};
use nova_common::{
    config::{load_config, DatabaseConfig, ObservabilityConfig, RedisConfig, ServerConfig},
    db, health, logging, redis_client,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let db_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging("delivery-worker", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "delivery-worker"));

    tracing::info!("Starting Nova Mail Delivery Worker v{}", VERSION);

    let db_pool = db::create_pool(&db_config.url, db_config.max_connections).await?;
    let redis = redis_client::create_client(&redis_config.url).await?;

    let state = AppState {
        db_pool: db_pool.clone(),
        redis: redis.clone(),
    };

    // Start background workers
    for i in 0..num_cpus::get() {
        let worker_state = state.clone();
        tokio::spawn(async move {
            tracing::info!("Starting worker thread {}", i);
            if let Err(e) = worker::run_worker(worker_state).await {
                tracing::error!("Worker {} error: {}", i, e);
            }
        });
    }

    // Health check server
    let app = Router::new()
        .route("/healthz", get(health::liveness_handler))
        .route(
            "/readyz",
            get(|| async move {
                let mut health_status = health::HealthStatus::new(VERSION);

                let db_check = match db::health_check(&db_pool).await {
                    Ok(_) => health::CheckResult::healthy(),
                    Err(e) => health::CheckResult::unhealthy(format!("Database: {}", e)),
                };
                health_status.add_check("database", db_check);

                health_status
            }),
        )
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("Delivery Worker health server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Delivery Worker shutdown complete");
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
