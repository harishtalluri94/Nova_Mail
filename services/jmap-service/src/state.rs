use nova_common::blob_storage::BlobStorage;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub storage: Arc<BlobStorage>,
}
