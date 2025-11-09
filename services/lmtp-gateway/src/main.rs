mod lmtp_server;
mod quota;

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
    pub storage: Arc<BlobStorage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let db_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging("lmtp-gateway", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "lmtp-gateway"));

    tracing::info!("Starting Nova Mail LMTP Gateway v{}", VERSION);

    let db_pool = db::create_pool(&db_config.url, db_config.max_connections).await?;
    let redis = redis_client::create_client(&redis_config.url).await?;

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
        storage,
    };

    // Start LMTP server on port 24
    let lmtp_addr = SocketAddr::from(([0, 0, 0, 0], 24));
    let lmtp_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("LMTP server listening on {}", lmtp_addr);
        if let Err(e) = lmtp_server::run_server(lmtp_addr, lmtp_state).await {
            tracing::error!("LMTP server error: {}", e);
        }
    });

    // Start health check HTTP server
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

    let http_addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("Health server listening on {}", http_addr);

    let listener = tokio::net::TcpListener::bind(http_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("LMTP Gateway shutdown complete");
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
