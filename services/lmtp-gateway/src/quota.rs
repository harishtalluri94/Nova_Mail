use crate::AppState;
use anyhow::Result;
use uuid::Uuid;

/// Check if user has available quota
pub async fn check_quota(user_id: Uuid, state: &AppState) -> Result<bool> {
    let quota = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"
        SELECT storage_used_bytes, storage_limit_bytes, message_count, message_limit
        FROM quotas
        WHERE user_id = $1
        "#
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let Some((storage_used, storage_limit, message_count, message_limit)) = quota else {
        // No quota record, check user defaults
        let user = sqlx::query_as::<_, (i64, i32)>(
            r#"
            SELECT storage_quota_bytes, message_quota
            FROM users
            WHERE id = $1
            "#
        )
        .bind(user_id)
        .fetch_one(&state.db_pool)
        .await?;

        // Initialize quota record
        sqlx::query(
            r#"
            INSERT INTO quotas (user_id, storage_limit_bytes, message_limit)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id) DO NOTHING
            "#
        )
        .bind(user_id)
        .bind(user.0)
        .bind(user.1 as i64)
        .execute(&state.db_pool)
        .await?;

        return Ok(true); // New user, has quota available
    };

    // Check storage quota
    if storage_used >= storage_limit {
        tracing::warn!("User {} storage quota exceeded", user_id);
        return Ok(false);
    }

    // Check message quota
    if message_count >= message_limit {
        tracing::warn!("User {} message quota exceeded", user_id);
        return Ok(false);
    }

    Ok(true)
}

/// Update quota after message delivery
pub async fn update_quota(user_id: Uuid, message_size: i64, state: &AppState) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE quotas
        SET storage_used_bytes = storage_used_bytes + $2,
            message_count = message_count + 1,
            updated_at = NOW()
        WHERE user_id = $1
        "#
    )
    .bind(user_id)
    .bind(message_size)
    .execute(&state.db_pool)
    .await?;

    Ok(())
}
