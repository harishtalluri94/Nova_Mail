use crate::error::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub fn init_logging(service_name: &str, otlp_endpoint: Option<&str>) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,nova_mail=debug"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json();

    let registry = Registry::default().with(env_filter).with(fmt_layer);

    // Add OpenTelemetry tracing if endpoint is provided
    if let Some(endpoint) = otlp_endpoint {
        use opentelemetry::trace::TracerProvider;
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .with_trace_config(sdktrace::config().with_resource(Resource::new(vec![
                opentelemetry::KeyValue::new("service.name", service_name.to_string()),
            ])))
            .install_batch(runtime::Tokio)
            .expect("Failed to initialize tracer")
            .tracer(service_name.to_string());

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        registry.with(telemetry_layer).try_init()?;
    } else {
        registry.try_init()?;
    }

    tracing::info!("Logging initialized for service: {}", service_name);

    Ok(())
}

pub fn init_sentry(dsn: &str, service_name: &str) -> sentry::ClientInitGuard {
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
