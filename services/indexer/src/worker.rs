use crate::text_extractor;
use crate::AppState;
use anyhow::{Context, Result};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexJob {
    pub message_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub blob_id: String,
}

const INDEX_QUEUE: &str = "nova:index:queue";
const BATCH_SIZE: usize = 100;

pub async fn run_worker(state: AppState) -> Result<()> {
    tracing::info!("Starting indexer worker");

    loop {
        match process_batch(&state).await {
            Ok(processed) => {
                if processed > 0 {
                    tracing::debug!("Processed {} messages", processed);
                }
            }
            Err(e) => {
                tracing::error!("Error processing batch: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }

        // Small delay between batches
        if let Err(e) = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            tokio::time::sleep(tokio::time::Duration::from_millis(100)),
        )
        .await
        {
            tracing::trace!("Batch delay interrupted: {}", e);
        }
    }
}

async fn process_batch(state: &AppState) -> Result<usize> {
    let mut redis = state.redis.clone();
    let mut processed = 0;

    // Pop jobs from queue
    for _ in 0..BATCH_SIZE {
        let job_data: Option<String> = redis
            .rpop(INDEX_QUEUE, None)
            .await
            .context("Failed to pop from queue")?;

        let Some(job_data) = job_data else {
            break;
        };

        let job: IndexJob =
            serde_json::from_str(&job_data).context("Failed to deserialize job")?;

        if let Err(e) = process_job(state, &job).await {
            tracing::error!(
                "Failed to process job for message {}: {}",
                job.message_id,
                e
            );
            // TODO: Add to dead-letter queue
        } else {
            processed += 1;
        }
    }

    // Commit indices for all tenants that had updates
    if processed > 0 {
        // In a real implementation, we'd track which tenants were updated
        // For now, we'll skip the commit here and rely on periodic commits
    }

    Ok(processed)
}

async fn process_job(state: &AppState, job: &IndexJob) -> Result<()> {
    tracing::debug!("Processing job for message {}", job.message_id);

    // Fetch message blob from object storage (already decompressed and verified)
    let raw_email = state.storage.retrieve_blob(&job.blob_id).await?;

    // Extract content
    let content = text_extractor::extract_content(&raw_email)?;

    // Get message metadata from database
    let message_meta = fetch_message_metadata(state, job.message_id).await?;

    // Index the message
    state.index_manager.index_message(
        job.tenant_id,
        job.message_id,
        &content.subject,
        &content.from_addr,
        &content.to_addrs,
        &content.cc_addrs,
        &content.body_text,
        content.body_html.as_deref(),
        message_meta.received_at,
        !content.attachments.is_empty(),
    )?;

    // Commit for this tenant
    state.index_manager.commit(job.tenant_id)?;

    tracing::info!("Indexed message {} for tenant {}", job.message_id, job.tenant_id);

    Ok(())
}

struct MessageMetadata {
    received_at: chrono::DateTime<chrono::Utc>,
}

async fn fetch_message_metadata(state: &AppState, message_id: Uuid) -> Result<MessageMetadata> {
    let row = sqlx::query_as::<_, (chrono::DateTime<chrono::Utc>,)>(
        r#"
        SELECT received_at
        FROM messages
        WHERE id = $1
        LIMIT 1
        "#
    )
    .bind(message_id)
    .fetch_one(&state.db_pool)
    .await
    .context("Failed to fetch message metadata")?;

    Ok(MessageMetadata {
        received_at: row.0,
    })
}

/// Enqueue a message for indexing
pub async fn enqueue_index_job(
    redis: &mut redis::aio::ConnectionManager,
    job: &IndexJob,
) -> Result<()> {
    let job_json = serde_json::to_string(job)?;
    redis
        .lpush(INDEX_QUEUE, job_json)
        .await
        .context("Failed to enqueue job")?;
    Ok(())
}
