use crate::handlers::SearchError;
use crate::query_parser::SearchQuery;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tantivy::{
    collector::TopDocs, schema::*, Index, IndexReader, ReloadPolicy,
};
use uuid::Uuid;

pub struct SearchService {
    base_path: PathBuf,
    readers: RwLock<HashMap<Uuid, IndexReader>>,
}

pub struct SearchHit {
    pub message_id: String,
    pub subject: String,
    pub from: String,
    pub received_at: i64,
    pub score: f32,
    pub snippet: Option<String>,
}

impl SearchService {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            base_path: base_path.as_ref().to_path_buf(),
            readers: RwLock::new(HashMap::new()),
        })
    }

    pub async fn search(
        &self,
        tenant_id: Uuid,
        query: &SearchQuery,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SearchHit>, SearchError> {
        // Get or create reader for tenant
        let reader = self
            .get_or_create_reader(tenant_id)
            .map_err(|e| SearchError::SearchError(e.to_string()))?;

        let searcher = reader.searcher();

        // Execute search
        let top_docs = searcher
            .search(&*query.query, &TopDocs::with_limit(limit + offset))
            .map_err(|e| SearchError::SearchError(e.to_string()))?;

        // Skip offset results
        let results: Vec<_> = top_docs.into_iter().skip(offset).collect();

        let schema = searcher.index().schema();
        let message_id_field = schema.get_field("message_id").unwrap();
        let subject_field = schema.get_field("subject").unwrap();
        let from_field = schema.get_field("from").unwrap();
        let received_at_field = schema.get_field("received_at").unwrap();

        let mut hits = Vec::new();

        for (score, doc_address) in results {
            let doc = searcher
                .doc(doc_address)
                .map_err(|e| SearchError::SearchError(e.to_string()))?;

            let message_id = doc
                .get_first(message_id_field)
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();

            let subject = doc
                .get_first(subject_field)
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();

            let from = doc
                .get_first(from_field)
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();

            let received_at = doc
                .get_first(received_at_field)
                .and_then(|v| v.as_date())
                .map(|dt| dt.into_timestamp_secs())
                .unwrap_or(0);

            hits.push(SearchHit {
                message_id,
                subject,
                from,
                received_at,
                score,
                snippet: None, // TODO: Generate snippet from body
            });
        }

        Ok(hits)
    }

    fn get_or_create_reader(&self, tenant_id: Uuid) -> Result<IndexReader> {
        // Check if already exists
        {
            let readers = self.readers.read().unwrap();
            if let Some(reader) = readers.get(&tenant_id) {
                return Ok(reader.clone());
            }
        }

        // Create new reader
        let index_path = self.base_path.join(tenant_id.to_string());

        if !index_path.exists() {
            anyhow::bail!("Index not found for tenant {}", tenant_id);
        }

        let index = Index::open_in_dir(&index_path).context("Failed to open index")?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()
            .context("Failed to create reader")?;

        // Store in map
        let mut readers = self.readers.write().unwrap();
        readers.insert(tenant_id, reader.clone());

        tracing::info!("Created reader for tenant {}", tenant_id);

        Ok(reader)
    }
}
