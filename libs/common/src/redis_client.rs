use crate::error::{Error, Result};
use redis::{aio::ConnectionManager, Client};

pub async fn create_client(redis_url: &str) -> Result<ConnectionManager> {
    let client = Client::open(redis_url).map_err(Error::Redis)?;
    let conn = ConnectionManager::new(client)
        .await
        .map_err(Error::Redis)?;
    Ok(conn)
}

pub async fn health_check(conn: &mut ConnectionManager) -> Result<()> {
    redis::cmd("PING")
        .query_async(conn)
        .await
        .map_err(Error::Redis)?;
    Ok(())
}
