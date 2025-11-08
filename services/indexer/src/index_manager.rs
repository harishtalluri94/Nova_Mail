use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter};
use uuid::Uuid;

pub struct IndexManager {
    base_path: PathBuf,
    indices: RwLock<HashMap<Uuid, TenantIndex>>,
}

pub struct TenantIndex {
    pub index: Index,
    pub writer: IndexWriter,
    pub schema: Schema,
}

impl IndexManager {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_path)?;

        Ok(Self {
            base_path,
            indices: RwLock::new(HashMap::new()),
        })
    }

    /// Get or create index for a tenant
    pub fn get_or_create_index(&self, tenant_id: Uuid) -> Result<()> {
        // Check if already exists
        {
            let indices = self.indices.read().unwrap();
            if indices.contains_key(&tenant_id) {
                return Ok(());
            }
        }

        // Create new index
        let index_path = self.base_path.join(tenant_id.to_string());
        std::fs::create_dir_all(&index_path)?;

        let schema = build_email_schema();
        let index = Index::create_in_dir(&index_path, schema.clone())
            .or_else(|_| Index::open_in_dir(&index_path))
            .context("Failed to create/open index")?;

        // Create writer with 50MB heap
        let writer = index
            .writer(50_000_000)
            .context("Failed to create index writer")?;

        let tenant_index = TenantIndex {
            index,
            writer,
            schema,
        };

        // Store in map
        let mut indices = self.indices.write().unwrap();
        indices.insert(tenant_id, tenant_index);

        tracing::info!("Created index for tenant {}", tenant_id);

        Ok(())
    }

    /// Index a message
    pub fn index_message(
        &self,
        tenant_id: Uuid,
        message_id: Uuid,
        subject: &str,
        from_addr: &str,
        to_addrs: &[String],
        cc_addrs: &[String],
        body_text: &str,
        body_html: Option<&str>,
        received_at: chrono::DateTime<chrono::Utc>,
        has_attachments: bool,
    ) -> Result<()> {
        self.get_or_create_index(tenant_id)?;

        let indices = self.indices.read().unwrap();
        let tenant_index = indices
            .get(&tenant_id)
            .context("Tenant index not found")?;

        let schema = &tenant_index.schema;

        // Get fields
        let message_id_field = schema.get_field("message_id").unwrap();
        let subject_field = schema.get_field("subject").unwrap();
        let from_field = schema.get_field("from").unwrap();
        let to_field = schema.get_field("to").unwrap();
        let cc_field = schema.get_field("cc").unwrap();
        let body_field = schema.get_field("body").unwrap();
        let received_at_field = schema.get_field("received_at").unwrap();
        let has_attachments_field = schema.get_field("has_attachments").unwrap();

        // Build document
        let mut doc = Document::new();

        doc.add_text(message_id_field, message_id.to_string());
        doc.add_text(subject_field, subject);
        doc.add_text(from_field, from_addr);

        for to in to_addrs {
            doc.add_text(to_field, to);
        }

        for cc in cc_addrs {
            doc.add_text(cc_field, cc);
        }

        // Combine text and HTML body
        let mut full_body = body_text.to_string();
        if let Some(html) = body_html {
            // Simple HTML tag stripping
            let text = strip_html_tags(html);
            full_body.push('\n');
            full_body.push_str(&text);
        }
        doc.add_text(body_field, &full_body);

        doc.add_date(
            received_at_field,
            tantivy::DateTime::from_timestamp_secs(received_at.timestamp()),
        );

        doc.add_bool(has_attachments_field, has_attachments);

        // Add document
        drop(indices); // Release read lock
        let mut indices = self.indices.write().unwrap();
        let tenant_index = indices.get_mut(&tenant_id).unwrap();
        tenant_index.writer.add_document(doc)?;

        Ok(())
    }

    /// Commit all pending changes
    pub fn commit(&self, tenant_id: Uuid) -> Result<()> {
        let mut indices = self.indices.write().unwrap();
        let tenant_index = indices
            .get_mut(&tenant_id)
            .context("Tenant index not found")?;

        tenant_index.writer.commit()?;

        tracing::debug!("Committed index for tenant {}", tenant_id);

        Ok(())
    }

    /// Delete a message from index
    pub fn delete_message(&self, tenant_id: Uuid, message_id: Uuid) -> Result<()> {
        let indices = self.indices.read().unwrap();
        let tenant_index = indices
            .get(&tenant_id)
            .context("Tenant index not found")?;

        let schema = &tenant_index.schema;
        let message_id_field = schema.get_field("message_id").unwrap();

        drop(indices);

        let mut indices = self.indices.write().unwrap();
        let tenant_index = indices.get_mut(&tenant_id).unwrap();

        let term = Term::from_field_text(message_id_field, &message_id.to_string());
        tenant_index.writer.delete_term(term);

        Ok(())
    }

    /// Health check
    pub fn is_healthy(&self) -> bool {
        // Simple check - can we read the indices map?
        self.indices.read().is_ok()
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        self.indices.read().unwrap().len()
    }
}

fn build_email_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Message ID (unique identifier)
    schema_builder.add_text_field("message_id", STRING | STORED);

    // Email headers
    schema_builder.add_text_field("subject", TEXT | STORED);
    schema_builder.add_text_field("from", TEXT | STORED);
    schema_builder.add_text_field("to", TEXT | STORED);
    schema_builder.add_text_field("cc", TEXT | STORED);

    // Body content (full-text searchable)
    schema_builder.add_text_field("body", TEXT);

    // Metadata
    schema_builder.add_date_field("received_at", INDEXED | STORED | FAST);
    schema_builder.add_bool_field("has_attachments", INDEXED | STORED);

    schema_builder.build()
}

fn strip_html_tags(html: &str) -> String {
    // Simple HTML tag stripping (production would use proper HTML parser)
    let mut result = String::new();
    let mut inside_tag = false;

    for c in html.chars() {
        match c {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ => {
                if !inside_tag {
                    result.push(c);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.index_count(), 0);
    }

    #[test]
    fn test_index_message() {
        let temp_dir = TempDir::new().unwrap();
        let manager = IndexManager::new(temp_dir.path()).unwrap();

        let tenant_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();

        manager
            .index_message(
                tenant_id,
                message_id,
                "Test Subject",
                "sender@example.com",
                &["recipient@example.com".to_string()],
                &[],
                "Test body content",
                None,
                chrono::Utc::now(),
                false,
            )
            .unwrap();

        manager.commit(tenant_id).unwrap();

        assert_eq!(manager.index_count(), 1);
    }
}
