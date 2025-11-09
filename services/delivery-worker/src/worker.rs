use crate::AppState;
use anyhow::{Context, Result};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeliveryJob {
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub mailbox_id: Uuid,
    pub blob_id: String,
}

const DELIVERY_QUEUE: &str = "nova:delivery:queue";
const BATCH_SIZE: usize = 100;

pub async fn run_worker(state: AppState) -> Result<()> {
    tracing::info!("Delivery worker started");

    loop {
        match process_batch(&state).await {
            Ok(processed) => {
                if processed > 0 {
                    tracing::debug!("Processed {} delivery jobs", processed);
                }
            }
            Err(e) => {
                tracing::error!("Error processing batch: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }

        // Small delay between batches
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

async fn process_batch(state: &AppState) -> Result<usize> {
    let mut redis = state.redis.clone();
    let mut processed = 0;

    for _ in 0..BATCH_SIZE {
        let job_data: Option<String> = redis
            .rpop(DELIVERY_QUEUE, None)
            .await
            .context("Failed to pop from queue")?;

        let Some(job_data) = job_data else {
            break;
        };

        let job: DeliveryJob =
            serde_json::from_str(&job_data).context("Failed to deserialize job")?;

        if let Err(e) = process_delivery_job(state, &job).await {
            tracing::error!(
                "Failed to process delivery for message {}: {}",
                job.message_id,
                e
            );
            // TODO: Add to dead-letter queue
        } else {
            processed += 1;
        }
    }

    Ok(processed)
}

async fn process_delivery_job(state: &AppState, job: &DeliveryJob) -> Result<()> {
    tracing::debug!("Processing delivery for message {}", job.message_id);

    // 1. Check for duplicates by blob hash
    let is_duplicate = check_duplicate(state, job).await?;

    if is_duplicate {
        tracing::info!("Message {} is a duplicate, skipping", job.message_id);
        return Ok(());
    }

    // 2. Detect thread (by In-Reply-To, References, or subject)
    let thread_id = detect_thread(state, job).await?;

    // 3. Update message with thread_id
    update_message_thread(state, job.message_id, thread_id).await?;

    // 4. Apply rules/filters
    apply_rules(state, job).await?;

    // 5. Update mailbox counts
    update_mailbox_counts(state, job.mailbox_id).await?;

    // 6. Increment modseq for mailbox state tracking
    let new_modseq = increment_modseq(state, job.mailbox_id).await?;

    // 7. Track message state change for JMAP delta queries
    track_message_state_change(state, job.mailbox_id, job.message_id, "created", new_modseq).await?;

    tracing::info!("Completed delivery for message {}", job.message_id);

    Ok(())
}

async fn check_duplicate(state: &AppState, job: &DeliveryJob) -> Result<bool> {
    // Check if blob_id already exists for this user
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM messages
        WHERE user_id = $1 AND blob_id = $2 AND id != $3
        "#,
    )
    .bind(job.user_id)
    .bind(&job.blob_id)
    .bind(job.message_id)
    .fetch_one(&state.db_pool)
    .await?;

    Ok(count > 0)
}

async fn detect_thread(state: &AppState, job: &DeliveryJob) -> Result<Uuid> {
    // Try to find existing thread by Message-ID references
    // For now, just return a new thread ID
    // TODO: Implement proper threading logic based on In-Reply-To and References headers

    Ok(Uuid::new_v4())
}

async fn update_message_thread(
    state: &AppState,
    message_id: Uuid,
    thread_id: Uuid,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE messages
        SET thread_id = $1
        WHERE id = $2
        "#,
        thread_id,
        message_id
    )
    .execute(&state.db_pool)
    .await?;

    Ok(())
}

async fn apply_rules(state: &AppState, job: &DeliveryJob) -> Result<()> {
    // Fetch rules for this user
    let rules = sqlx::query!(
        r#"
        SELECT id, conditions, actions, priority
        FROM rules
        WHERE user_id = $1 AND is_enabled = true
        ORDER BY priority ASC
        "#,
        job.user_id
    )
    .fetch_all(&state.db_pool)
    .await?;

    for rule in rules {
        // TODO: Evaluate rule conditions against message
        // TODO: Apply rule actions (move to mailbox, add labels, mark as read, etc.)
        tracing::debug!("Rule {} evaluation not yet implemented", rule.id);
    }

    Ok(())
}

async fn update_mailbox_counts(state: &AppState, mailbox_id: Uuid) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE mailboxes
        SET
            total_messages = (SELECT COUNT(*) FROM messages WHERE mailbox_id = $1 AND is_deleted = false),
            unread_messages = (SELECT COUNT(*) FROM messages WHERE mailbox_id = $1 AND is_deleted = false AND is_read = false),
            updated_at = NOW()
        WHERE id = $1
        "#,
        mailbox_id
    )
    .execute(&state.db_pool)
    .await?;

    Ok(())
}

async fn increment_modseq(state: &AppState, mailbox_id: Uuid) -> Result<i64> {
    // Increment modification sequence for JMAP state tracking
    // This uses the increment_mailbox_modseq function from the database migration

    let new_modseq = sqlx::query_scalar!(
        "SELECT increment_mailbox_modseq($1)",
        mailbox_id
    )
    .fetch_one(&state.db_pool)
    .await
    .context("Failed to increment mailbox modseq")?
    .unwrap_or(0);

    tracing::debug!(
        "Incremented modseq for mailbox {} to {}",
        mailbox_id,
        new_modseq
    );

    Ok(new_modseq)
}

async fn track_message_state_change(
    state: &AppState,
    mailbox_id: Uuid,
    message_id: Uuid,
    change_type: &str,
    modseq: i64,
) -> Result<()> {
    // Track message state changes for JMAP delta queries
    // This uses the track_message_change function from the database migration

    sqlx::query!(
        "SELECT track_message_change($1, $2, $3, $4, $5)",
        mailbox_id,
        message_id,
        change_type,
        modseq,
        None::<serde_json::Value> // changed_fields (null for creation)
    )
    .execute(&state.db_pool)
    .await
    .context("Failed to track message state change")?;

    tracing::debug!(
        "Tracked {} for message {} with modseq {}",
        change_type,
        message_id,
        modseq
    );

    Ok(())
}

/// Enqueue a delivery job
pub async fn enqueue_delivery_job(
    redis: &mut redis::aio::ConnectionManager,
    job: &DeliveryJob,
) -> Result<()> {
    let job_json = serde_json::to_string(job)?;
    redis
        .lpush(DELIVERY_QUEUE, job_json)
        .await
        .context("Failed to enqueue delivery job")?;
    Ok(())
}
