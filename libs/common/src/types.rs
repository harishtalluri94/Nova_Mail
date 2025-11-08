use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "tenant_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    Active,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub status: TenantStatus,
    pub max_users: i32,
    pub max_storage_gb: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub domain: String,
    pub dkim_selector: String,
    pub dkim_private_key: String, // Encrypted
    pub spf_record: String,
    pub dmarc_record: String,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mailbox {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub role: MailboxRole,
    pub parent_id: Option<Uuid>,
    pub sort_order: i32,
    pub total_messages: i64,
    pub unread_messages: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "mailbox_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MailboxRole {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Spam,
    Archive,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub user_id: Uuid,
    pub subject: String,
    pub from_addr: String,
    pub to_addrs: Vec<String>,
    pub cc_addrs: Vec<String>,
    pub bcc_addrs: Vec<String>,
    pub size_bytes: i64,
    pub has_attachments: bool,
    pub is_read: bool,
    pub is_flagged: bool,
    pub is_draft: bool,
    pub blob_id: String, // Content-addressed ID in object storage
    pub received_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub user_id: Uuid,
    pub storage_used_bytes: i64,
    pub storage_limit_bytes: i64,
    pub message_count: i64,
    pub message_limit: i64,
    pub updated_at: DateTime<Utc>,
}

impl Quota {
    pub fn is_over_storage_limit(&self) -> bool {
        self.storage_used_bytes >= self.storage_limit_bytes
    }

    pub fn is_over_message_limit(&self) -> bool {
        self.message_count >= self.message_limit
    }

    pub fn remaining_storage_bytes(&self) -> i64 {
        self.storage_limit_bytes.saturating_sub(self.storage_used_bytes)
    }
}
