mod handlers;
mod routes;

use axum::Router;
use nova_common::{
    config::{load_config, DatabaseConfig, ObservabilityConfig, RedisConfig, ServerConfig},
    db, health, logging, redis_client,
};
use std::net::SocketAddr;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let db_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    // Initialize logging and observability
    logging::init_logging(
        "admin-api",
        Some(&observability_config.otlp_endpoint),
    )?;

    // Initialize Sentry if DSN is provided
    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "admin-api"));

    tracing::info!("Starting Nova Mail Admin API v{}", VERSION);

    // Initialize database pool
    tracing::info!("Connecting to database...");
    let db_pool = db::create_pool(&db_config.url, db_config.max_connections).await?;

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await?;

    // Initialize Redis
    tracing::info!("Connecting to Redis...");
    let redis = redis_client::create_client(&redis_config.url).await?;

    let state = AppState {
        db_pool: db_pool.clone(),
        redis,
    };

    // Build application router
    let app = Router::new()
        .merge(routes::api_routes(state.clone()))
        .merge(routes::health_routes(state.clone()))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("Admin API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Admin API shutdown complete");
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
