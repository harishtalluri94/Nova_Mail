use crate::error::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub device_type: DeviceType,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Web,
    Mobile,
    Desktop,
    Imap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub token: String,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

impl AccessToken {
    pub fn new(user_id: Uuid, device_id: Uuid, expiry_seconds: i64) -> Self {
        let token = format!("{}_{}", Uuid::new_v4(), Uuid::new_v4());
        let expires_at = Utc::now() + Duration::seconds(expiry_seconds);

        Self {
            token,
            user_id,
            device_id,
            expires_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Extracts bearer token from Authorization header
pub fn extract_bearer_token(auth_header: &str) -> Result<&str> {
    auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| Error::Authentication("Invalid authorization header".to_string()))
}

/// Validates that user has access to a specific tenant
pub fn validate_tenant_access(user_tenant_id: Uuid, requested_tenant_id: Uuid) -> Result<()> {
    if user_tenant_id != requested_tenant_id {
        return Err(Error::Authorization(
            "Access denied to this tenant".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnCredential {
    pub id: Vec<u8>,
    pub user_id: Uuid,
    pub public_key: Vec<u8>,
    pub counter: u32,
    pub created_at: DateTime<Utc>,
}
