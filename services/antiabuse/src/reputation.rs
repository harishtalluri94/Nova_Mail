use anyhow::Result;
use sqlx::PgPool;
use std::net::IpAddr;

#[derive(Debug)]
pub struct ReputationResult {
    pub score: f32,
    pub blocklisted: bool,
    pub allowlisted: bool,
    pub reasons: Vec<String>,
}

pub struct ReputationService {
    db: PgPool,
}

impl ReputationService {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Check reputation of IP, domain, or email
    pub async fn check(
        &self,
        ip_address: &Option<String>,
        domain: &Option<String>,
        email: &Option<String>,
    ) -> Result<ReputationResult> {
        let mut score = 1.0; // Start with good reputation
        let mut reasons = Vec::new();
        let mut blocklisted = false;
        let mut allowlisted = false;

        // Check IP reputation
        if let Some(ip) = ip_address {
            let ip_result = self.check_ip(ip).await?;
            if ip_result.blocklisted {
                blocklisted = true;
                score = 0.0;
                reasons.push(format!("IP {} is blocklisted", ip));
            } else if ip_result.allowlisted {
                allowlisted = true;
                reasons.push(format!("IP {} is allowlisted", ip));
            } else {
                score *= ip_result.score;
                reasons.extend(ip_result.reasons);
            }
        }

        // Check domain reputation
        if let Some(dom) = domain {
            let domain_result = self.check_domain(dom).await?;
            if domain_result.blocklisted {
                blocklisted = true;
                score = 0.0;
                reasons.push(format!("Domain {} is blocklisted", dom));
            } else if domain_result.allowlisted {
                allowlisted = true;
                reasons.push(format!("Domain {} is allowlisted", dom));
            } else {
                score *= domain_result.score;
                reasons.extend(domain_result.reasons);
            }
        }

        // Check email reputation
        if let Some(em) = email {
            let email_result = self.check_email(em).await?;
            if email_result.blocklisted {
                blocklisted = true;
                score = 0.0;
                reasons.push(format!("Email {} is blocklisted", em));
            } else if email_result.allowlisted {
                allowlisted = true;
                reasons.push(format!("Email {} is allowlisted", em));
            } else {
                score *= email_result.score;
                reasons.extend(email_result.reasons);
            }
        }

        Ok(ReputationResult {
            score,
            blocklisted,
            allowlisted,
            reasons,
        })
    }

    async fn check_ip(&self, ip: &str) -> Result<ReputationResult> {
        // Check local blocklist/allowlist first
        let blocklist_entry = sqlx::query!(
            r#"
            SELECT reason FROM ip_blocklist
            WHERE ip_address = $1 AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            ip
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(entry) = blocklist_entry {
            return Ok(ReputationResult {
                score: 0.0,
                blocklisted: true,
                allowlisted: false,
                reasons: vec![entry.reason],
            });
        }

        let allowlist_entry = sqlx::query!(
            r#"
            SELECT reason FROM ip_allowlist WHERE ip_address = $1
            "#,
            ip
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(entry) = allowlist_entry {
            return Ok(ReputationResult {
                score: 1.0,
                blocklisted: false,
                allowlisted: true,
                reasons: vec![entry.reason],
            });
        }

        // Check if IP is valid
        let ip_addr: IpAddr = ip.parse()?;
        let mut score = 1.0;
        let mut reasons = Vec::new();

        // Reduce score for dynamic/residential IPs
        // This is a simplified check - in production, use GeoIP database
        if self.is_likely_dynamic(&ip_addr) {
            score *= 0.8;
            reasons.push("IP appears to be dynamic/residential".to_string());
        }

        // TODO: Check external reputation services
        // - Spamhaus ZEN
        // - Barracuda
        // - Proofpoint Emerging Threats
        // - AbuseIPDB

        Ok(ReputationResult {
            score,
            blocklisted: false,
            allowlisted: false,
            reasons,
        })
    }

    async fn check_domain(&self, domain: &str) -> Result<ReputationResult> {
        // Check local blocklist/allowlist
        let blocklist_entry = sqlx::query!(
            r#"
            SELECT reason FROM domain_blocklist
            WHERE domain = $1 AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            domain
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(entry) = blocklist_entry {
            return Ok(ReputationResult {
                score: 0.0,
                blocklisted: true,
                allowlisted: false,
                reasons: vec![entry.reason],
            });
        }

        let allowlist_entry = sqlx::query!(
            r#"
            SELECT reason FROM domain_allowlist WHERE domain = $1
            "#,
            domain
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(entry) = allowlist_entry {
            return Ok(ReputationResult {
                score: 1.0,
                blocklisted: false,
                allowlisted: true,
                reasons: vec![entry.reason],
            });
        }

        // Check domain age, WHOIS, DNS records, etc.
        // For now, return neutral score
        Ok(ReputationResult {
            score: 0.9,
            blocklisted: false,
            allowlisted: false,
            reasons: vec![],
        })
    }

    async fn check_email(&self, email: &str) -> Result<ReputationResult> {
        // Check local blocklist/allowlist
        let blocklist_entry = sqlx::query!(
            r#"
            SELECT reason FROM email_blocklist
            WHERE email = $1 AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            email
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(entry) = blocklist_entry {
            return Ok(ReputationResult {
                score: 0.0,
                blocklisted: true,
                allowlisted: false,
                reasons: vec![entry.reason],
            });
        }

        let allowlist_entry = sqlx::query!(
            r#"
            SELECT reason FROM email_allowlist WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(entry) = allowlist_entry {
            return Ok(ReputationResult {
                score: 1.0,
                blocklisted: false,
                allowlisted: true,
                reasons: vec![entry.reason],
            });
        }

        // Check for disposable email domains
        let domain = email.split('@').nth(1).unwrap_or("");
        if self.is_disposable_domain(domain) {
            return Ok(ReputationResult {
                score: 0.3,
                blocklisted: false,
                allowlisted: false,
                reasons: vec!["Disposable email domain".to_string()],
            });
        }

        Ok(ReputationResult {
            score: 0.9,
            blocklisted: false,
            allowlisted: false,
            reasons: vec![],
        })
    }

    fn is_likely_dynamic(&self, _ip: &IpAddr) -> bool {
        // In production, use MaxMind GeoIP database or similar
        // to detect residential/dynamic IPs
        false
    }

    fn is_disposable_domain(&self, domain: &str) -> bool {
        // List of known disposable email domains
        const DISPOSABLE_DOMAINS: &[&str] = &[
            "tempmail.com",
            "guerrillamail.com",
            "10minutemail.com",
            "mailinator.com",
            "throwaway.email",
            "temp-mail.org",
        ];

        DISPOSABLE_DOMAINS.contains(&domain)
    }

    /// Add to blocklist
    pub async fn add_to_blocklist(
        &self,
        identifier: &str,
        identifier_type: &crate::handlers::BlocklistType,
        reason: &str,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        match identifier_type {
            crate::handlers::BlocklistType::Ip => {
                sqlx::query!(
                    r#"
                    INSERT INTO ip_blocklist (ip_address, reason, expires_at, created_at)
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (ip_address) DO UPDATE SET reason = $2, expires_at = $3
                    "#,
                    identifier,
                    reason,
                    expires_at
                )
                .execute(&self.db)
                .await?;
            }
            crate::handlers::BlocklistType::Domain => {
                sqlx::query!(
                    r#"
                    INSERT INTO domain_blocklist (domain, reason, expires_at, created_at)
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (domain) DO UPDATE SET reason = $2, expires_at = $3
                    "#,
                    identifier,
                    reason,
                    expires_at
                )
                .execute(&self.db)
                .await?;
            }
            crate::handlers::BlocklistType::Email => {
                sqlx::query!(
                    r#"
                    INSERT INTO email_blocklist (email, reason, expires_at, created_at)
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (email) DO UPDATE SET reason = $2, expires_at = $3
                    "#,
                    identifier,
                    reason,
                    expires_at
                )
                .execute(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    /// Remove from blocklist
    pub async fn remove_from_blocklist(&self, identifier: &str) -> Result<()> {
        // Try removing from all blocklist tables
        sqlx::query!("DELETE FROM ip_blocklist WHERE ip_address = $1", identifier)
            .execute(&self.db)
            .await?;

        sqlx::query!("DELETE FROM domain_blocklist WHERE domain = $1", identifier)
            .execute(&self.db)
            .await?;

        sqlx::query!("DELETE FROM email_blocklist WHERE email = $1", identifier)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    /// Add to allowlist
    pub async fn add_to_allowlist(
        &self,
        identifier: &str,
        identifier_type: &crate::handlers::BlocklistType,
        reason: &str,
    ) -> Result<()> {
        match identifier_type {
            crate::handlers::BlocklistType::Ip => {
                sqlx::query!(
                    r#"
                    INSERT INTO ip_allowlist (ip_address, reason, created_at)
                    VALUES ($1, $2, NOW())
                    ON CONFLICT (ip_address) DO UPDATE SET reason = $2
                    "#,
                    identifier,
                    reason
                )
                .execute(&self.db)
                .await?;
            }
            crate::handlers::BlocklistType::Domain => {
                sqlx::query!(
                    r#"
                    INSERT INTO domain_allowlist (domain, reason, created_at)
                    VALUES ($1, $2, NOW())
                    ON CONFLICT (domain) DO UPDATE SET reason = $2
                    "#,
                    identifier,
                    reason
                )
                .execute(&self.db)
                .await?;
            }
            crate::handlers::BlocklistType::Email => {
                sqlx::query!(
                    r#"
                    INSERT INTO email_allowlist (email, reason, created_at)
                    VALUES ($1, $2, NOW())
                    ON CONFLICT (email) DO UPDATE SET reason = $2
                    "#,
                    identifier,
                    reason
                )
                .execute(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    /// Remove from allowlist
    pub async fn remove_from_allowlist(&self, identifier: &str) -> Result<()> {
        // Try removing from all allowlist tables
        sqlx::query!("DELETE FROM ip_allowlist WHERE ip_address = $1", identifier)
            .execute(&self.db)
            .await?;

        sqlx::query!("DELETE FROM domain_allowlist WHERE domain = $1", identifier)
            .execute(&self.db)
            .await?;

        sqlx::query!("DELETE FROM email_allowlist WHERE email = $1", identifier)
            .execute(&self.db)
            .await?;

        Ok(())
    }
}
