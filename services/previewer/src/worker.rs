use crate::generators;
use crate::AppState;
use anyhow::{Context, Result};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewJob {
    pub attachment_id: Uuid,
    pub blob_id: String,
    pub content_type: String,
    pub filename: String,
}

const PREVIEW_QUEUE: &str = "nova:preview:queue";

pub async fn run_worker(state: AppState) -> Result<()> {
    tracing::info!("Preview worker started");

    loop {
        match process_job(&state).await {
            Ok(processed) => {
                if processed {
                    tracing::debug!("Processed preview job");
                }
            }
            Err(e) => {
                tracing::error!("Error processing job: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

async fn process_job(state: &AppState) -> Result<bool> {
    let mut redis = state.redis.clone();

    let job_data: Option<String> = redis
        .rpop(PREVIEW_QUEUE, None)
        .await
        .context("Failed to pop from queue")?;

    let Some(job_data) = job_data else {
        return Ok(false);
    };

    let job: PreviewJob =
        serde_json::from_str(&job_data).context("Failed to deserialize job")?;

    if let Err(e) = generate_preview(state, &job).await {
        tracing::error!(
            "Failed to generate preview for attachment {}: {}",
            job.attachment_id,
            e
        );
        // TODO: Add to dead-letter queue
    }

    Ok(true)
}

async fn generate_preview(state: &AppState, job: &PreviewJob) -> Result<()> {
    tracing::info!("Generating preview for {}", job.filename);

    // Fetch blob from object storage
    let blob_data = fetch_blob(&job.blob_id).await?;

    // Determine preview type based on content type
    let preview_blob_id = if job.content_type.starts_with("image/") {
        // Generate image thumbnail
        let thumbnail = generators::generate_image_thumbnail(&blob_data, &job.content_type)?;
        upload_preview(&thumbnail, "image/webp").await?
    } else if job.content_type == "application/pdf" {
        // Generate PDF preview (first 3 pages)
        let previews = generators::generate_pdf_preview(&blob_data, 3)?;
        if let Some(first_page) = previews.first() {
            upload_preview(first_page, "image/png").await?
        } else {
            tracing::warn!("No PDF preview generated");
            return Ok(());
        }
    } else if is_office_document(&job.content_type) {
        // Generate Office document preview
        let text_preview = generators::generate_office_preview(&blob_data, &job.content_type)?;
        upload_preview(text_preview.as_bytes(), "text/plain").await?
    } else {
        tracing::debug!("No preview generation for content type: {}", job.content_type);
        return Ok(());
    };

    // Update attachment record with preview blob ID
    sqlx::query!(
        r#"
        UPDATE attachments
        SET preview_blob_id = $1
        WHERE id = $2
        "#,
        preview_blob_id,
        job.attachment_id
    )
    .execute(&state.db_pool)
    .await?;

    tracing::info!(
        "Generated preview for attachment {} -> {}",
        job.attachment_id,
        preview_blob_id
    );

    Ok(())
}

async fn fetch_blob(blob_id: &str) -> Result<Vec<u8>> {
    // TODO: Implement actual S3/R2 fetch
    tracing::warn!("Blob fetch not implemented for {}", blob_id);
    Ok(vec![])
}

async fn upload_preview(data: &[u8], content_type: &str) -> Result<String> {
    // TODO: Implement actual S3/R2 upload to previews bucket
    let preview_id = format!("preview_{}", Uuid::new_v4());
    tracing::debug!("Uploaded {} bytes as {} -> {}", data.len(), content_type, preview_id);
    Ok(preview_id)
}

fn is_office_document(content_type: &str) -> bool {
    matches!(
        content_type,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" // docx
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" // xlsx
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation" // pptx
        | "application/msword" // doc
        | "application/vnd.ms-excel" // xls
        | "application/vnd.ms-powerpoint" // ppt
    )
}

/// Enqueue a preview generation job
pub async fn enqueue_preview_job(
    redis: &mut redis::aio::ConnectionManager,
    job: &PreviewJob,
) -> Result<()> {
    let job_json = serde_json::to_string(job)?;
    redis
        .lpush(PREVIEW_QUEUE, job_json)
        .await
        .context("Failed to enqueue preview job")?;
    Ok(())
}
