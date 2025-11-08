use crate::error::{Error, Result};
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectStorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    pub otlp_endpoint: String,
    pub service_name: String,
    pub metrics_port: u16,
    pub sentry_dsn: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiry: u64,
    pub refresh_token_expiry: u64,
    pub webauthn_origin: String,
    pub webauthn_rp_id: String,
}

/// Load configuration from files and environment variables
pub fn load_config(config_path: Option<&Path>) -> Result<Config> {
    let mut builder = Config::builder();

    // Load from default config file if provided
    if let Some(path) = config_path {
        builder = builder.add_source(File::from(path).required(false));
    }

    // Load from environment-specific config
    let env = std::env::var("ENV").unwrap_or_else(|_| "development".into());
    builder = builder.add_source(File::with_name(&format!("config/{}", env)).required(false));

    // Load from environment variables (with prefix NOVA_)
    builder = builder.add_source(
        Environment::with_prefix("NOVA")
            .separator("__")
            .try_parsing(true),
    );

    // Load .env file
    dotenvy::dotenv().ok();

    builder
        .build()
        .map_err(|e| Error::Config(format!("Failed to load configuration: {}", e)))
}

/// Helper to deserialize specific config section
pub fn get_config_section<'de, T: Deserialize<'de>>(
    config: &'de Config,
    section: &str,
) -> Result<T> {
    config
        .get::<T>(section)
        .map_err(|e: ConfigError| Error::Config(format!("Failed to get {}: {}", section, e)))
}
