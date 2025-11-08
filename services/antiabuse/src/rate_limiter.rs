use anyhow::Result;
use redis::AsyncCommands;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub limit: u64,
    pub remaining: u64,
    pub reset_at: i64,
}

pub struct RateLimiter {
    redis: redis::aio::ConnectionManager,
    limits: HashMap<String, RateLimit>,
}

#[derive(Debug, Clone)]
struct RateLimit {
    max_requests: u64,
    window_seconds: u64,
}

impl RateLimiter {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        let mut limits = HashMap::new();

        // Define rate limits for different actions
        limits.insert(
            "send_email".to_string(),
            RateLimit {
                max_requests: 100,
                window_seconds: 3600, // 100 emails per hour
            },
        );
        limits.insert(
            "send_email_burst".to_string(),
            RateLimit {
                max_requests: 10,
                window_seconds: 60, // 10 emails per minute
            },
        );
        limits.insert(
            "signup".to_string(),
            RateLimit {
                max_requests: 5,
                window_seconds: 3600, // 5 signups per hour per IP
            },
        );
        limits.insert(
            "login".to_string(),
            RateLimit {
                max_requests: 20,
                window_seconds: 900, // 20 logins per 15 minutes
            },
        );
        limits.insert(
            "login_failed".to_string(),
            RateLimit {
                max_requests: 5,
                window_seconds: 300, // 5 failed logins per 5 minutes
            },
        );
        limits.insert(
            "api_call".to_string(),
            RateLimit {
                max_requests: 1000,
                window_seconds: 60, // 1000 API calls per minute
            },
        );

        Self { redis, limits }
    }

    /// Check if a request should be rate limited
    pub async fn check(
        &self,
        user_id: &Option<Uuid>,
        ip_address: &Option<String>,
        action: &str,
    ) -> Result<RateLimitResult> {
        // Determine the identifier for rate limiting
        let identifier = self.get_identifier(user_id, ip_address);

        // Get the rate limit configuration for this action
        let limit = self.limits.get(action).ok_or_else(|| {
            anyhow::anyhow!("Unknown action: {}", action)
        })?;

        // Check both the main limit and burst limit for send_email
        if action == "send_email" {
            // Check burst limit first
            let burst_result = self
                .check_limit(&identifier, "send_email_burst", limit)
                .await?;
            if !burst_result.allowed {
                return Ok(burst_result);
            }
        }

        // Check the main limit
        self.check_limit(&identifier, action, limit).await
    }

    async fn check_limit(
        &self,
        identifier: &str,
        action: &str,
        limit: &RateLimit,
    ) -> Result<RateLimitResult> {
        let key = format!("ratelimit:{}:{}", action, identifier);
        let now = chrono::Utc::now().timestamp();
        let window_start = now - limit.window_seconds as i64;
        let reset_at = now + limit.window_seconds as i64;

        let mut conn = self.redis.clone();

        // Use sorted sets to track requests in a sliding window
        // Remove old requests
        let _: () = conn
            .zrembyscore(&key, "-inf", window_start)
            .await?;

        // Count requests in current window
        let count: u64 = conn.zcard(&key).await?;

        let allowed = count < limit.max_requests;
        let remaining = if allowed {
            limit.max_requests - count - 1
        } else {
            0
        };

        if allowed {
            // Add current request
            let request_id = Uuid::new_v4().to_string();
            let _: () = conn.zadd(&key, request_id, now).await?;

            // Set expiration on the key
            let _: () = conn.expire(&key, limit.window_seconds as i64).await?;
        }

        Ok(RateLimitResult {
            allowed,
            limit: limit.max_requests,
            remaining,
            reset_at,
        })
    }

    fn get_identifier(&self, user_id: &Option<Uuid>, ip_address: &Option<String>) -> String {
        match (user_id, ip_address) {
            (Some(uid), _) => format!("user:{}", uid),
            (None, Some(ip)) => format!("ip:{}", ip),
            (None, None) => "anonymous".to_string(),
        }
    }

    /// Manually consume tokens (useful for tracking without checking)
    pub async fn consume(
        &self,
        user_id: &Option<Uuid>,
        ip_address: &Option<String>,
        action: &str,
    ) -> Result<()> {
        let identifier = self.get_identifier(user_id, ip_address);
        let limit = self.limits.get(action).ok_or_else(|| {
            anyhow::anyhow!("Unknown action: {}", action)
        })?;

        let key = format!("ratelimit:{}:{}", action, identifier);
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.redis.clone();

        let request_id = Uuid::new_v4().to_string();
        let _: () = conn.zadd(&key, request_id, now).await?;
        let _: () = conn.expire(&key, limit.window_seconds as i64).await?;

        Ok(())
    }

    /// Reset rate limit for an identifier
    pub async fn reset(
        &self,
        user_id: &Option<Uuid>,
        ip_address: &Option<String>,
        action: &str,
    ) -> Result<()> {
        let identifier = self.get_identifier(user_id, ip_address);
        let key = format!("ratelimit:{}:{}", action, identifier);

        let mut conn = self.redis.clone();
        let _: () = conn.del(&key).await?;

        Ok(())
    }

    /// Get current usage statistics
    pub async fn get_usage(
        &self,
        user_id: &Option<Uuid>,
        ip_address: &Option<String>,
        action: &str,
    ) -> Result<(u64, u64)> {
        let identifier = self.get_identifier(user_id, ip_address);
        let limit = self.limits.get(action).ok_or_else(|| {
            anyhow::anyhow!("Unknown action: {}", action)
        })?;

        let key = format!("ratelimit:{}:{}", action, identifier);
        let now = chrono::Utc::now().timestamp();
        let window_start = now - limit.window_seconds as i64;

        let mut conn = self.redis.clone();

        // Remove old requests
        let _: () = conn
            .zrembyscore(&key, "-inf", window_start)
            .await?;

        // Count requests in current window
        let count: u64 = conn.zcard(&key).await?;

        Ok((count, limit.max_requests))
    }
}
