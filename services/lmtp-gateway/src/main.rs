use nova_common::logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init_logging("lmtp-gateway", None)?;
    tracing::info!("Nova Mail LMTP Gateway starting...");

    // TODO: Implement LMTP server
    // - Accept LMTP connections from edge-mta
    // - Check quotas
    // - Store message blobs (zstd compressed, content-addressed)
    // - Write metadata to PostgreSQL
    // - Enqueue indexing and preview jobs

    tracing::warn!("LMTP Gateway not yet implemented");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
