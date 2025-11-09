use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

// Stub implementation of IndexManager for compilation
// TODO: Implement full-text search with tantivy or alternative search solution
pub struct IndexManager {
    _base_path: std::path::PathBuf,
}

impl IndexManager {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_path)?;

        tracing::warn!("IndexManager initialized in stub mode - search functionality disabled");

        Ok(Self {
            _base_path: base_path,
        })
    }

    /// Index a message (stub - does nothing)
    #[allow(clippy::too_many_arguments)]
    pub fn index_message(
        &self,
        _tenant_id: Uuid,
        message_id: Uuid,
        _subject: &str,
        _from_addr: &str,
        _to_addrs: &[String],
        _cc_addrs: &[String],
        _body_text: &str,
        _body_html: Option<&str>,
        _received_at: chrono::DateTime<chrono::Utc>,
        _has_attachments: bool,
    ) -> Result<()> {
        tracing::debug!("Stub: Would index message {}", message_id);
        Ok(())
    }

    /// Commit all pending changes (stub - does nothing)
    pub fn commit(&self, tenant_id: Uuid) -> Result<()> {
        tracing::debug!("Stub: Would commit index for tenant {}", tenant_id);
        Ok(())
    }

    /// Delete a message from index (stub - does nothing)
    pub fn delete_message(&self, _tenant_id: Uuid, message_id: Uuid) -> Result<()> {
        tracing::debug!("Stub: Would delete message {}", message_id);
        Ok(())
    }

    /// Health check
    pub fn is_healthy(&self) -> bool {
        true
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        0
    }
}
