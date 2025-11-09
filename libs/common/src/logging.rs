use crate::error::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub fn init_logging(service_name: &str, _otlp_endpoint: Option<&str>) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,nova_mail=debug"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json();

    let registry = Registry::default().with(env_filter).with(fmt_layer);

    // Note: OpenTelemetry integration temporarily disabled due to API compatibility
    // TODO: Re-enable with correct OpenTelemetry SDK version
    registry.try_init().map_err(|_e| {
        crate::error::Error::Internal("Failed to initialize logging".to_string())
    })?;

    tracing::info!("Logging initialized for service: {}", service_name);

    Ok(())
}

pub fn init_sentry(dsn: &str, _service_name: &str) -> sentry::ClientInitGuard {
    sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(env!("CARGO_PKG_VERSION").into()),
            environment: Some(
                std::env::var("ENV")
                    .unwrap_or_else(|_| "development".into())
                    .into(),
            ),
            traces_sample_rate: 0.1,
            ..Default::default()
        },
    ))
}
