mod handlers;
mod jmap;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use nova_common::{
    blob_storage::BlobStorage,
    config::{load_config, DatabaseConfig, ObservabilityConfig, RedisConfig, ServerConfig},
    db, health, logging, redis_client,
};
use std::sync::Arc;
use state::AppState;
use std::net::SocketAddr;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let db_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging("jmap-service", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "jmap-service"));

    tracing::info!("Starting Nova Mail JMAP Service v{}", VERSION);

    let db_pool = db::create_pool(&db_config.url, db_config.max_connections).await?;
    let redis = redis_client::create_client(&redis_config.url).await?;

    // Initialize blob storage
    let s3_endpoint = std::env::var("S3_ENDPOINT").ok();
    let s3_bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "nova-mail-blobs".to_string());
    let storage = Arc::new(BlobStorage::new(s3_endpoint, s3_bucket).await?);

    let state = AppState {
        db_pool: db_pool.clone(),
        redis,
        storage,
    };

    // Build router
    let app = Router::new()
        // JMAP endpoints
        .route("/jmap", post(handlers::jmap_request))
        .route("/jmap/session", get(handlers::session))
        .route("/jmap/upload/:account_id", post(handlers::upload))
        .route("/jmap/download/:blob_id", get(handlers::download))

        // Health endpoints
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
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("JMAP Service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("JMAP Service shutdown complete");
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
