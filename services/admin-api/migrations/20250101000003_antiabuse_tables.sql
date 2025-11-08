-- IP Blocklist and Allowlist
CREATE TABLE ip_blocklist (
    ip_address VARCHAR(45) PRIMARY KEY,
    reason TEXT NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE ip_allowlist (
    ip_address VARCHAR(45) PRIMARY KEY,
    reason TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ip_blocklist_expires ON ip_blocklist(expires_at) WHERE expires_at IS NOT NULL;

-- Domain Blocklist and Allowlist
CREATE TABLE domain_blocklist (
    domain VARCHAR(255) PRIMARY KEY,
    reason TEXT NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE domain_allowlist (
    domain VARCHAR(255) PRIMARY KEY,
    reason TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_domain_blocklist_expires ON domain_blocklist(expires_at) WHERE expires_at IS NOT NULL;

-- Email Blocklist and Allowlist
CREATE TABLE email_blocklist (
    email VARCHAR(255) PRIMARY KEY,
    reason TEXT NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE email_allowlist (
    email VARCHAR(255) PRIMARY KEY,
    reason TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_email_blocklist_expires ON email_blocklist(expires_at) WHERE expires_at IS NOT NULL;
