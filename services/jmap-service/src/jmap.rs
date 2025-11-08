use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JMAP Session object
/// Spec: https://jmap.io/spec-core.html#the-jmap-session-resource
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JmapSession {
    pub capabilities: Capabilities,
    pub accounts: Vec<Account>,
    pub primary_accounts: PrimaryAccounts,
    pub username: String,
    pub api_url: String,
    pub download_url: String,
    pub upload_url: String,
    pub event_source_url: String,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(rename = "urn:ietf:params:jmap:core")]
    pub core: CoreCapability,
    #[serde(rename = "urn:ietf:params:jmap:mail")]
    pub mail: MailCapability,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreCapability {
    pub max_size_upload: u64,
    pub max_concurrent_upload: u32,
    pub max_size_request: u64,
    pub max_concurrent_requests: u32,
    pub max_calls_in_request: u32,
    pub max_objects_in_get: u32,
    pub max_objects_in_set: u32,
    pub collation_algorithms: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailCapability {
    pub max_mailboxes_per_email: Option<u32>,
    pub max_mailbox_depth: Option<u32>,
    pub max_size_mailbox_name: u32,
    pub max_size_attachments_per_email: u64,
    pub email_query_sort_options: Vec<String>,
    pub may_create_top_level_mailbox: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    #[serde(rename = "isPersonal")]
    pub is_personal: bool,
    #[serde(rename = "isReadOnly")]
    pub is_read_only: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrimaryAccounts {
    #[serde(rename = "urn:ietf:params:jmap:mail")]
    pub mail: String,
}

/// JMAP Request object
/// Spec: https://jmap.io/spec-core.html#the-request-object
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JmapRequest {
    pub using: HashMap<String, String>,
    pub method_calls: Vec<MethodCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_ids: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MethodCall {
    #[serde(rename = "0")]
    pub method: String,
    #[serde(rename = "1")]
    pub arguments: serde_json::Value,
    #[serde(rename = "2")]
    pub call_id: String,
}

/// JMAP Response object
/// Spec: https://jmap.io/spec-core.html#the-response-object
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JmapResponse {
    pub method_responses: Vec<MethodResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MethodResponse {
    #[serde(rename = "0")]
    pub method: String,
    #[serde(rename = "1")]
    pub result: serde_json::Value,
    #[serde(rename = "2")]
    pub call_id: String,
}

/// Email object (simplified)
/// Spec: https://jmap.io/spec-mail.html#email-objects
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Email {
    pub id: String,
    pub blob_id: String,
    pub thread_id: String,
    pub mailbox_ids: HashMap<String, bool>,
    pub keywords: HashMap<String, bool>,
    pub size: u64,
    pub received_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec<EmailAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec<EmailAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_attachment: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
}

/// Mailbox object
/// Spec: https://jmap.io/spec-mail.html#mailboxes
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mailbox {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub sort_order: u32,
    pub total_emails: u64,
    pub unread_emails: u64,
    pub total_threads: u64,
    pub unread_threads: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub my_rights: Option<MailboxRights>,
    pub is_subscribed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxRights {
    pub may_read_items: bool,
    pub may_add_items: bool,
    pub may_remove_items: bool,
    pub may_set_seen: bool,
    pub may_set_keywords: bool,
    pub may_create_child: bool,
    pub may_rename: bool,
    pub may_delete: bool,
    pub may_submit: bool,
}
