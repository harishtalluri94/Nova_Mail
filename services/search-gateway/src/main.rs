mod handlers;
mod query_parser;
mod searcher;

use axum::{
    routing::{get, post},
    Router,
};
use nova_common::{
    config::{load_config, ObservabilityConfig, ServerConfig},
    health, logging,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub searcher: Arc<searcher::SearchService>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging(
        "search-gateway",
        Some(&observability_config.otlp_endpoint),
    )?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "search-gateway"));

    tracing::info!("Starting Nova Mail Search Gateway v{}", VERSION);

    // Initialize search service
    let index_path = std::env::var("INDEX_PATH").unwrap_or_else(|_| "./indices".to_string());
    let searcher = Arc::new(searcher::SearchService::new(&index_path)?);

    let state = AppState { searcher };

    // Build router
    let app = Router::new()
        .route("/api/v1/search", post(handlers::search))
        .route("/healthz", get(health::liveness_handler))
        .route("/readyz", get(health::liveness_handler))
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
    tracing::info!("Search Gateway listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Search Gateway shutdown complete");
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
