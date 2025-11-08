mod handlers;
mod model;

use axum::{
    routing::{get, post},
    Router,
};
use nova_common::{
    config::{load_config, ObservabilityConfig, RedisConfig, ServerConfig},
    health, logging, redis_client,
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
    pub redis: redis::aio::ConnectionManager,
    pub model: Arc<model::ModelEngine>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config(None)?;
    let server_config: ServerConfig = config.get("server")?;
    let redis_config: RedisConfig = config.get("redis")?;
    let observability_config: ObservabilityConfig = config.get("observability")?;

    logging::init_logging("ai-assistant", Some(&observability_config.otlp_endpoint))?;

    let _guard = observability_config
        .sentry_dsn
        .as_ref()
        .map(|dsn| logging::init_sentry(dsn, "ai-assistant"));

    tracing::info!("Starting Nova Mail AI Assistant v{}", VERSION);

    let redis = redis_client::create_client(&redis_config.url).await?;

    // Initialize model engine
    tracing::info!("Loading AI model...");
    let model_path = std::env::var("AI_MODEL_PATH")
        .unwrap_or_else(|_| "./models/mistral-7b-instruct.gguf".to_string());

    // Get a multiplexed connection for the model cache
    let redis_client = redis::Client::open(redis_config.url.clone())?;
    let redis_multiplex = redis_client.get_multiplexed_async_connection().await.ok();

    let model = Arc::new(model::ModelEngine::new(&model_path, redis_multiplex));
    tracing::info!("AI model loaded successfully");

    let state = AppState { redis, model };

    // Build router
    let app = Router::new()
        .route("/api/v1/compose", post(handlers::compose))
        .route("/api/v1/rewrite", post(handlers::rewrite))
        .route("/api/v1/summarize", post(handlers::summarize))
        .route("/api/v1/extract_actions", post(handlers::extract_actions))
        .route("/healthz", get(health::liveness_handler))
        .route("/readyz", get(health::liveness_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], server_config.port));
    tracing::info!("AI Assistant listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("AI Assistant shutdown complete");
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
