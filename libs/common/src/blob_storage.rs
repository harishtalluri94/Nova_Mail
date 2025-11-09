use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use blake3::Hasher;
use tokio::io::AsyncReadExt;

/// Blob storage service with content-addressed storage
/// Uses BLAKE3 for hashing and zstd for compression
pub struct BlobStorage {
    s3_client: S3Client,
    bucket: String,
}

impl BlobStorage {
    /// Create a new blob storage instance
    pub async fn new(endpoint: Option<String>, bucket: String) -> Result<Self> {
        let config = if let Some(endpoint_url) = endpoint {
            // Custom endpoint (for R2, MinIO, or LocalStack)
            let config = aws_config::defaults(BehaviorVersion::latest())
                .endpoint_url(endpoint_url)
                .load()
                .await;
            config
        } else {
            // Standard AWS S3
            aws_config::load_defaults(BehaviorVersion::latest()).await
        };

        let s3_client = S3Client::new(&config);

        tracing::info!("Initialized blob storage with bucket: {}", bucket);

        Ok(Self { s3_client, bucket })
    }

    /// Store a blob with content-addressed storage
    /// Returns the BLAKE3 hash as the blob ID
    pub async fn store_blob(&self, data: &[u8]) -> Result<String> {
        // Compute BLAKE3 hash (content-addressed key)
        let hash = Self::compute_hash(data);

        // Check if blob already exists (deduplication)
        if self.blob_exists(&hash).await? {
            tracing::debug!("Blob {} already exists, skipping upload", hash);
            return Ok(hash);
        }

        // Compress data with zstd
        let compressed = Self::compress(data)?;

        tracing::info!(
            "Storing blob: hash={}, original_size={}, compressed_size={}, ratio={:.2}%",
            hash,
            data.len(),
            compressed.len(),
            100.0 - (compressed.len() as f64 / data.len() as f64 * 100.0)
        );

        // Upload to S3/R2
        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(&hash)
            .body(compressed.into())
            .content_type("application/octet-stream")
            .metadata("original-size", data.len().to_string())
            .metadata("compression", "zstd")
            .send()
            .await
            .context("Failed to upload blob to S3")?;

        tracing::debug!("Successfully stored blob: {}", hash);
        Ok(hash)
    }

    /// Retrieve a blob by its hash
    pub async fn retrieve_blob(&self, blob_id: &str) -> Result<Vec<u8>> {
        tracing::debug!("Retrieving blob: {}", blob_id);

        // Download from S3/R2
        let response = self
            .s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(blob_id)
            .send()
            .await
            .context("Failed to download blob from S3")?;

        // Read the body
        let mut compressed = Vec::new();
        let mut body = response.body.into_async_read();
        body.read_to_end(&mut compressed)
            .await
            .context("Failed to read blob body")?;

        tracing::debug!(
            "Downloaded blob: {}, compressed_size={}",
            blob_id,
            compressed.len()
        );

        // Decompress
        let decompressed = Self::decompress(&compressed)?;

        // Verify hash
        let computed_hash = Self::compute_hash(&decompressed);
        if computed_hash != blob_id {
            anyhow::bail!(
                "Hash mismatch: expected {}, got {}",
                blob_id,
                computed_hash
            );
        }

        tracing::debug!(
            "Successfully retrieved and verified blob: {}, size={}",
            blob_id,
            decompressed.len()
        );

        Ok(decompressed)
    }

    /// Check if a blob exists
    pub async fn blob_exists(&self, blob_id: &str) -> Result<bool> {
        match self
            .s3_client
            .head_object()
            .bucket(&self.bucket)
            .key(blob_id)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("404") || err_str.contains("NotFound") {
                    Ok(false)
                } else {
                    Err(err).context("Failed to check blob existence")
                }
            }
        }
    }

    /// Delete a blob (for cleanup)
    pub async fn delete_blob(&self, blob_id: &str) -> Result<()> {
        self.s3_client
            .delete_object()
            .bucket(&self.bucket)
            .key(blob_id)
            .send()
            .await
            .context("Failed to delete blob from S3")?;

        tracing::info!("Deleted blob: {}", blob_id);
        Ok(())
    }

    /// Get blob metadata without downloading
    pub async fn get_blob_metadata(&self, blob_id: &str) -> Result<BlobMetadata> {
        let response = self
            .s3_client
            .head_object()
            .bucket(&self.bucket)
            .key(blob_id)
            .send()
            .await
            .context("Failed to get blob metadata")?;

        let original_size = response
            .metadata()
            .and_then(|m| m.get("original-size"))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        let compressed_size = response.content_length().unwrap_or(0) as usize;

        Ok(BlobMetadata {
            blob_id: blob_id.to_string(),
            original_size,
            compressed_size,
            compression: response
                .metadata()
                .and_then(|m| m.get("compression").map(|s| s.to_string()))
                .unwrap_or_else(|| "none".to_string()),
        })
    }

    /// Compute BLAKE3 hash of data (content-addressed ID)
    pub fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hex::encode(hasher.finalize().as_bytes())
    }

    /// Compress data with zstd (level 3 - balanced speed/compression)
    fn compress(data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(data, 3).context("Failed to compress data")
    }

    /// Decompress zstd data
    fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data).context("Failed to decompress data")
    }

    /// List all blobs (for debugging/admin)
    pub async fn list_blobs(&self, prefix: Option<&str>, max_keys: i32) -> Result<Vec<String>> {
        let mut request = self.s3_client.list_objects_v2().bucket(&self.bucket);

        if let Some(p) = prefix {
            request = request.prefix(p);
        }

        let response = request
            .max_keys(max_keys)
            .send()
            .await
            .context("Failed to list blobs")?;

        let blobs = response
            .contents()
            .iter()
            .filter_map(|obj| obj.key().map(|k| k.to_string()))
            .collect();

        Ok(blobs)
    }

    /// Get total storage usage
    pub async fn get_storage_usage(&self) -> Result<StorageUsage> {
        let response = self
            .s3_client
            .list_objects_v2()
            .bucket(&self.bucket)
            .send()
            .await
            .context("Failed to list objects for usage calculation")?;

        let mut total_size = 0u64;
        let mut blob_count = 0u64;

        for obj in response.contents() {
            total_size += obj.size().unwrap_or(0) as u64;
            blob_count += 1;
        }

        Ok(StorageUsage {
            total_size_bytes: total_size,
            blob_count,
        })
    }
}

#[derive(Debug)]
pub struct BlobMetadata {
    pub blob_id: String,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression: String,
}

#[derive(Debug)]
pub struct StorageUsage {
    pub total_size_bytes: u64,
    pub blob_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_computation() {
        let data = b"Hello, World!";
        let hash = BlobStorage::compute_hash(data);

        // BLAKE3 hash should be deterministic
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(
            hash,
            "ede5c0b10f2ec4979c69b52f61e42ff5b413519ce09be0f14d098dcfe5f3f7c4"
        );
    }

    #[test]
    fn test_compression_decompression() {
        let data = b"This is a test message that should compress well. ".repeat(100);

        let compressed = BlobStorage::compress(&data).unwrap();
        let decompressed = BlobStorage::decompress(&compressed).unwrap();

        assert_eq!(data.to_vec(), decompressed);
        assert!(compressed.len() < data.len()); // Should be smaller
    }

    #[test]
    fn test_compression_ratio() {
        let data = b"A".repeat(10000);
        let compressed = BlobStorage::compress(&data).unwrap();

        let ratio = data.len() as f64 / compressed.len() as f64;
        println!("Compression ratio: {:.2}x", ratio);

        // Repeated data should compress very well
        assert!(ratio > 10.0);
    }

    #[test]
    fn test_hash_consistency() {
        let data = b"test data";
        let hash1 = BlobStorage::compute_hash(data);
        let hash2 = BlobStorage::compute_hash(data);
        assert_eq!(hash1, hash2);
    }
}
