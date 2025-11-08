mod handlers;
mod signer;

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
    pub db_pool: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub signer: Arc<signer::LinkSigner>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let db_config: DatabaseConfig = config.get("database")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging("link-service", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "link-service"));

    tracing::info!("Starting Nova Mail Link Service v{}", VERSION);

    let db_pool = db::create_pool(&db_config.url, db_config.max_connections).await?;
    let redis = redis_client::create_client(&redis_config.url).await?;

    // Get signing secret from environment
    let signing_secret =
        std::env::var("LINK_SIGNING_SECRET").unwrap_or_else(|_| "dev-secret-key".to_string());

    let signer = Arc::new(signer::LinkSigner::new(&signing_secret));

    let state = AppState {
        db_pool: db_pool.clone(),
        redis,
        signer,
    };

    let app = Router::new()
        .route("/api/v1/links", post(handlers::create_link))
        .route("/api/v1/links/:link_id", get(handlers::get_link_info))
        .route("/api/v1/links/:link_id/revoke", post(handlers::revoke_link))
        .route("/d/:token", get(handlers::download))
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
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("Link Service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Link Service shutdown complete");
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
