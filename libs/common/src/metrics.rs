use axum::{http::StatusCode, response::IntoResponse};
use opentelemetry::KeyValue;
use opentelemetry_prometheus::PrometheusExporter;
use opentelemetry_sdk::{
    metrics::{reader::DefaultAggregationSelector, MeterProvider, PeriodicReader},
    runtime, Resource,
};
use prometheus::{Encoder, TextEncoder};

pub fn init_metrics(service_name: &str) -> PrometheusExporter {
    let exporter = opentelemetry_prometheus::exporter()
        .with_resource(Resource::new(vec![KeyValue::new(
            "service.name",
            service_name.to_string(),
        )]))
        .build()
        .expect("Failed to create Prometheus exporter");

    opentelemetry::global::set_meter_provider(MeterProvider::builder().with_reader(exporter.clone()).build());

    exporter
}

pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];

    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (StatusCode::OK, buffer).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {}", e),
        )
            .into_response(),
    }
}
