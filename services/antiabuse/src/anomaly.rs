use anyhow::Result;
use redis::AsyncCommands;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct AnomalyResult {
    pub is_anomalous: bool,
    pub confidence: f32,
    pub reasons: Vec<String>,
}

pub struct AnomalyDetector {
    redis: redis::aio::ConnectionManager,
    thresholds: HashMap<String, AnomalyThreshold>,
}

#[derive(Debug, Clone)]
struct AnomalyThreshold {
    max_events_per_hour: u64,
    max_unique_recipients_per_hour: u64,
    max_email_size_kb: u64,
}

impl AnomalyDetector {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        let mut thresholds = HashMap::new();

        // Define thresholds for different event types
        thresholds.insert(
            "send_email".to_string(),
            AnomalyThreshold {
                max_events_per_hour: 200,
                max_unique_recipients_per_hour: 100,
                max_email_size_kb: 25 * 1024, // 25 MB
            },
        );
        thresholds.insert(
            "login".to_string(),
            AnomalyThreshold {
                max_events_per_hour: 50,
                max_unique_recipients_per_hour: 0,
                max_email_size_kb: 0,
            },
        );
        thresholds.insert(
            "api_call".to_string(),
            AnomalyThreshold {
                max_events_per_hour: 10000,
                max_unique_recipients_per_hour: 0,
                max_email_size_kb: 0,
            },
        );

        Self { redis, thresholds }
    }

    /// Detect anomalous behavior
    pub async fn detect(
        &self,
        user_id: &Uuid,
        event_type: &str,
        metadata: &Value,
    ) -> Result<AnomalyResult> {
        let mut is_anomalous = false;
        let mut confidence = 0.0;
        let mut reasons = Vec::new();

        // Get threshold for this event type
        let threshold = self.thresholds.get(event_type);

        // Check event frequency
        let frequency_result = self.check_frequency(user_id, event_type, threshold).await?;
        if frequency_result.0 {
            is_anomalous = true;
            confidence = confidence.max(frequency_result.1);
            reasons.push(frequency_result.2);
        }

        // Check for specific anomalies based on event type
        match event_type {
            "send_email" => {
                let email_results = self.check_email_anomalies(user_id, metadata, threshold).await?;
                if !email_results.is_empty() {
                    is_anomalous = true;
                    confidence = confidence.max(0.8);
                    reasons.extend(email_results);
                }
            }
            "login" => {
                let login_results = self.check_login_anomalies(user_id, metadata).await?;
                if !login_results.is_empty() {
                    is_anomalous = true;
                    confidence = confidence.max(0.7);
                    reasons.extend(login_results);
                }
            }
            _ => {}
        }

        // Track this event for future analysis
        self.track_event(user_id, event_type, metadata).await?;

        Ok(AnomalyResult {
            is_anomalous,
            confidence,
            reasons,
        })
    }

    /// Check if event frequency is anomalous
    async fn check_frequency(
        &self,
        user_id: &Uuid,
        event_type: &str,
        threshold: Option<&AnomalyThreshold>,
    ) -> Result<(bool, f32, String)> {
        let key = format!("anomaly:freq:{}:{}", user_id, event_type);
        let now = chrono::Utc::now().timestamp();
        let one_hour_ago = now - 3600;

        let mut conn = self.redis.clone();

        // Remove old events
        let _: () = conn.zrembyscore(&key, "-inf", one_hour_ago).await?;

        // Count events in last hour
        let count: u64 = conn.zcard(&key).await?;

        if let Some(thresh) = threshold {
            if count > thresh.max_events_per_hour {
                return Ok((
                    true,
                    0.9,
                    format!(
                        "Unusual frequency: {} {} events in last hour (threshold: {})",
                        count, event_type, thresh.max_events_per_hour
                    ),
                ));
            }
        }

        Ok((false, 0.0, String::new()))
    }

    /// Check for email-specific anomalies
    async fn check_email_anomalies(
        &self,
        user_id: &Uuid,
        metadata: &Value,
        threshold: Option<&AnomalyThreshold>,
    ) -> Result<Vec<String>> {
        let mut anomalies = Vec::new();

        // Check email size
        if let Some(size) = metadata.get("size_bytes").and_then(|v| v.as_u64()) {
            if let Some(thresh) = threshold {
                if size > thresh.max_email_size_kb * 1024 {
                    anomalies.push(format!(
                        "Unusually large email: {} KB",
                        size / 1024
                    ));
                }
            }
        }

        // Check number of recipients
        if let Some(recipients) = metadata.get("recipient_count").and_then(|v| v.as_u64()) {
            if recipients > 100 {
                anomalies.push(format!(
                    "Large number of recipients: {}",
                    recipients
                ));
            }
        }

        // Check for spam-like patterns
        if let Some(subject) = metadata.get("subject").and_then(|v| v.as_str()) {
            if self.looks_like_spam(subject) {
                anomalies.push("Subject contains spam-like patterns".to_string());
            }
        }

        // Check unique recipients in last hour
        if let Some(recipient) = metadata.get("recipient").and_then(|v| v.as_str()) {
            let unique_count = self
                .track_unique_recipient(user_id, recipient)
                .await?;

            if let Some(thresh) = threshold {
                if unique_count > thresh.max_unique_recipients_per_hour {
                    anomalies.push(format!(
                        "Too many unique recipients in last hour: {}",
                        unique_count
                    ));
                }
            }
        }

        Ok(anomalies)
    }

    /// Check for login-specific anomalies
    async fn check_login_anomalies(
        &self,
        user_id: &Uuid,
        metadata: &Value,
    ) -> Result<Vec<String>> {
        let mut anomalies = Vec::new();

        // Check for unusual location
        if let Some(country) = metadata.get("country").and_then(|v| v.as_str()) {
            let is_new_country = self.is_new_location(user_id, country).await?;
            if is_new_country {
                anomalies.push(format!("Login from new country: {}", country));
            }
        }

        // Check for unusual user agent
        if let Some(user_agent) = metadata.get("user_agent").and_then(|v| v.as_str()) {
            if self.is_suspicious_user_agent(user_agent) {
                anomalies.push("Suspicious user agent detected".to_string());
            }
        }

        // Check for rapid successive logins from different IPs
        if let Some(ip) = metadata.get("ip").and_then(|v| v.as_str()) {
            let rapid_from_different_ip = self
                .check_rapid_ip_change(user_id, ip)
                .await?;
            if rapid_from_different_ip {
                anomalies.push("Rapid login from different IP address".to_string());
            }
        }

        Ok(anomalies)
    }

    /// Track an event for future analysis
    async fn track_event(
        &self,
        user_id: &Uuid,
        event_type: &str,
        metadata: &Value,
    ) -> Result<()> {
        let key = format!("anomaly:freq:{}:{}", user_id, event_type);
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.redis.clone();

        // Add event with current timestamp as score
        let event_id = Uuid::new_v4().to_string();
        let _: () = conn.zadd(&key, event_id, now).await?;

        // Expire after 24 hours
        let _: () = conn.expire(&key, 86400).await?;

        // Store metadata for analysis
        let meta_key = format!("anomaly:meta:{}:{}", user_id, event_type);
        let meta_json = serde_json::to_string(metadata)?;
        let _: () = conn.set_ex(meta_key, meta_json, 86400).await?;

        Ok(())
    }

    /// Track unique recipients
    async fn track_unique_recipient(
        &self,
        user_id: &Uuid,
        recipient: &str,
    ) -> Result<u64> {
        let key = format!("anomaly:recipients:{}", user_id);
        let now = chrono::Utc::now().timestamp();
        let one_hour_ago = now - 3600;

        let mut conn = self.redis.clone();

        // Remove old recipients
        let _: () = conn.zrembyscore(&key, "-inf", one_hour_ago).await?;

        // Add current recipient
        let _: () = conn.zadd(&key, recipient, now).await?;

        // Count unique recipients
        let count: u64 = conn.zcard(&key).await?;

        // Expire after 24 hours
        let _: () = conn.expire(&key, 86400).await?;

        Ok(count)
    }

    /// Check if this is a new location for the user
    async fn is_new_location(
        &self,
        user_id: &Uuid,
        country: &str,
    ) -> Result<bool> {
        let key = format!("anomaly:locations:{}", user_id);

        let mut conn = self.redis.clone();

        // Check if country exists in set
        let exists: bool = conn.sismember(&key, country).await?;

        if !exists {
            // Add to set
            let _: () = conn.sadd(&key, country).await?;
            // Expire after 90 days
            let _: () = conn.expire(&key, 90 * 86400).await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if there's a rapid IP change
    async fn check_rapid_ip_change(
        &self,
        user_id: &Uuid,
        current_ip: &str,
    ) -> Result<bool> {
        let key = format!("anomaly:last_ip:{}", user_id);

        let mut conn = self.redis.clone();

        // Get last IP
        let last_ip: Option<String> = conn.get(&key).await?;

        // Set current IP
        let _: () = conn.set_ex(&key, current_ip, 300).await?; // 5 minute window

        if let Some(last) = last_ip {
            if last != current_ip {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn looks_like_spam(&self, subject: &str) -> bool {
        let spam_keywords = [
            "viagra",
            "cialis",
            "lottery",
            "winner",
            "congratulations",
            "claim your prize",
            "act now",
            "limited time",
            "free money",
            "work from home",
        ];

        let subject_lower = subject.to_lowercase();
        spam_keywords.iter().any(|&keyword| subject_lower.contains(keyword))
    }

    fn is_suspicious_user_agent(&self, user_agent: &str) -> bool {
        // Check for known bot patterns
        let suspicious_patterns = ["bot", "crawler", "scraper", "python-requests", "curl"];

        let ua_lower = user_agent.to_lowercase();
        suspicious_patterns.iter().any(|&pattern| ua_lower.contains(pattern))
    }

    /// Get user behavior profile
    pub async fn get_profile(&self, user_id: &Uuid) -> Result<BehaviorProfile> {
        let mut conn = self.redis.clone();

        // Get event frequencies
        let send_key = format!("anomaly:freq:{}:send_email", user_id);
        let login_key = format!("anomaly:freq:{}:login", user_id);

        let send_count: u64 = conn.zcard(&send_key).await?;
        let login_count: u64 = conn.zcard(&login_key).await?;

        // Get locations
        let location_key = format!("anomaly:locations:{}", user_id);
        let locations: Vec<String> = conn.smembers(&location_key).await?;

        Ok(BehaviorProfile {
            emails_last_hour: send_count,
            logins_last_hour: login_count,
            known_locations: locations,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct BehaviorProfile {
    pub emails_last_hour: u64,
    pub logins_last_hour: u64,
    pub known_locations: Vec<String>,
}

use serde::Serialize;
