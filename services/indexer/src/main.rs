mod index_manager;
mod text_extractor;
mod worker;

use axum::{routing::get, Router};
use nova_common::{
    blob_storage::BlobStorage,
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
    pub index_manager: Arc<index_manager::IndexManager>,
    pub storage: Arc<BlobStorage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let db_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    // Initialize logging
    logging::init_logging("indexer", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "indexer"));

    tracing::info!("Starting Nova Mail Indexer v{}", VERSION);

    // Initialize database pool
    let db_pool = db::create_pool(&db_config.url, db_config.max_connections).await?;

    // Initialize Redis
    let redis = redis_client::create_client(&redis_config.url).await?;

    // Initialize index manager
    let index_path = std::env::var("INDEX_PATH").unwrap_or_else(|_| "./indices".to_string());
    let index_manager = Arc::new(index_manager::IndexManager::new(&index_path)?);

    // Initialize blob storage
    let s3_endpoint = std::env::var("S3_ENDPOINT").ok();
    let s3_bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "nova-mail-blobs".to_string());

    tracing::info!(
        "Initializing blob storage: endpoint={:?}, bucket={}",
        s3_endpoint,
        s3_bucket
    );

    let storage = Arc::new(BlobStorage::new(s3_endpoint, s3_bucket).await?);

    let state = AppState {
        db_pool: db_pool.clone(),
        redis: redis.clone(),
        index_manager: index_manager.clone(),
        storage,
    };

    // Start background worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = worker::run_worker(worker_state).await {
            tracing::error!("Worker error: {}", e);
        }
    });

    // Build health check server
    let app = Router::new()
        .route("/healthz", get(health::liveness_handler))
        .route(
            "/readyz",
            get(|| async move {
                let mut health_status = health::HealthStatus::new(VERSION);

                // Check database
                let db_check = match nova_common::db::health_check(&db_pool).await {
                    Ok(_) => health::CheckResult::healthy(),
                    Err(e) => health::CheckResult::unhealthy(format!("Database: {}", e)),
                };
                health_status.add_check("database", db_check);

                // Check Redis
                let mut redis_conn = redis.clone();
                let redis_check = match nova_common::redis_client::health_check(&mut redis_conn).await {
                    Ok(_) => health::CheckResult::healthy(),
                    Err(e) => health::CheckResult::unhealthy(format!("Redis: {}", e)),
                };
                health_status.add_check("redis", redis_check);

                // Check index manager
                let index_check = if index_manager.is_healthy() {
                    health::CheckResult::healthy()
                } else {
                    health::CheckResult::degraded("Index manager degraded".to_string())
                };
                health_status.add_check("index_manager", index_check);

                health_status
            }),
        )
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("Indexer health server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Indexer shutdown complete");
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
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            tracing::info!("Received terminate signal");
        },
    }
}
