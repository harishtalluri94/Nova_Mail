use crate::AppState;
use anyhow::Result;
use uuid::Uuid;

/// Check if user has available quota
pub async fn check_quota(user_id: Uuid, state: &AppState) -> Result<bool> {
    let quota = sqlx::query!(
        r#"
        SELECT storage_used_bytes, storage_limit_bytes, message_count, message_limit
        FROM quotas
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&state.db_pool)
    .await?;

    let Some(quota) = quota else {
        // No quota record, check user defaults
        let user = sqlx::query!(
            r#"
            SELECT storage_quota_bytes, message_quota
            FROM users
            WHERE id = $1
            "#,
            user_id
        )
        .fetch_one(&state.db_pool)
        .await?;

        // Initialize quota record
        sqlx::query!(
            r#"
            INSERT INTO quotas (user_id, storage_limit_bytes, message_limit)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id) DO NOTHING
            "#,
            user_id,
            user.storage_quota_bytes,
            user.message_quota as i64
        )
        .execute(&state.db_pool)
        .await?;

        return Ok(true); // New user, has quota available
    };

    // Check storage quota
    if quota.storage_used_bytes >= quota.storage_limit_bytes {
        tracing::warn!("User {} storage quota exceeded", user_id);
        return Ok(false);
    }

    // Check message quota
    if quota.message_count >= quota.message_limit {
        tracing::warn!("User {} message quota exceeded", user_id);
        return Ok(false);
    }

    Ok(true)
}

/// Update quota after message delivery
pub async fn update_quota(user_id: Uuid, message_size: i64, state: &AppState) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE quotas
        SET storage_used_bytes = storage_used_bytes + $2,
            message_count = message_count + 1,
            updated_at = NOW()
        WHERE user_id = $1
        "#,
        user_id,
        message_size
    )
    .execute(&state.db_pool)
    .await?;

    Ok(())
}
