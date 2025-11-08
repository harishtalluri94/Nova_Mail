-- Add attachment_links table for link-service

CREATE TABLE attachment_links (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    blob_id VARCHAR(64) NOT NULL,
    filename VARCHAR(255) NOT NULL,
    content_type VARCHAR(255) NOT NULL,
    size_bytes BIGINT NOT NULL,
    download_count INTEGER NOT NULL DEFAULT 0,
    max_downloads INTEGER,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    revoked BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_attachment_links_blob ON attachment_links(blob_id);
CREATE INDEX idx_attachment_links_expires ON attachment_links(expires_at);
CREATE INDEX idx_attachment_links_revoked ON attachment_links(revoked) WHERE revoked = false;

-- Cleanup expired links (for periodic maintenance)
COMMENT ON TABLE attachment_links IS 'Time-limited signed links for attachment downloads';
