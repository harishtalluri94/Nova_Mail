# Nova Mail Architecture

## Overview

Nova Mail is a modern, cloud-native email platform built for performance, scalability, and cost-efficiency.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          Internet                                │
└───────────┬─────────────────────────────────┬───────────────────┘
            │                                 │
            │ SMTP (25/587/465)              │ HTTPS (443)
            │                                 │
┌───────────▼────────────┐        ┌──────────▼──────────────────┐
│   Edge MTA (VMs)       │        │   Cloudflare CDN            │
│   - Stalwart SMTP      │        │   - HTTP/3                  │
│   - DKIM Signing       │        │   - DDoS Protection         │
│   - Rspamd             │        └──────────┬──────────────────┘
│   - Static IPs         │                   │
└───────────┬────────────┘                   │
            │ LMTP                            │ HTTP/3
            │                                 │
┌───────────▼────────────┐        ┌──────────▼──────────────────┐
│   LMTP Gateway         │        │   JMAP Service              │
│   - Quota Check        │        │   - HTTP/3 + QUIC           │
│   - Blob Storage       │        │   - IMAP/Submission         │
│   - Metadata Write     │        │   - State Management        │
└───────────┬────────────┘        └──────────┬──────────────────┘
            │                                 │
            ├─────────────────────────────────┤
            │                                 │
┌───────────▼─────────────────────────────────▼───────────────────┐
│                    Core Data Layer                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  PostgreSQL  │  │    Redis     │  │   R2/MinIO   │          │
│  │  (Metadata)  │  │   (Cache)    │  │   (Blobs)    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└───────────────────────────────────────────────────────────────────┘
            │                                 │
┌───────────▼─────────────────────────────────▼───────────────────┐
│                   Worker Services                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Indexer    │  │  Previewer   │  │   Delivery   │          │
│  │  (Tantivy)   │  │   (WebP)     │  │   Worker     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└──────────────────────────────────────────────────────────────────┘
            │
┌───────────▼──────────────────────────────────────────────────────┐
│                   Supporting Services                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │    Search    │  │  AI Assistant│  │  Link Service│          │
│  │   Gateway    │  │   (7-8B LLM) │  │              │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│  ┌──────────────┐  ┌──────────────┐                            │
│  │  Admin API   │  │  AntiAbuse   │                            │
│  └──────────────┘  └──────────────┘                            │
└──────────────────────────────────────────────────────────────────┘
```

## Components

### Edge MTA (Virtual Machines)
- **Purpose**: SMTP ingress/egress, email authentication
- **Technology**: Stalwart Mail Server, Rspamd, ClamAV
- **Deployment**: 2 VMs with static IPs (not in K8s)
- **Key Features**:
  - SMTP (25), Submission (587/465)
  - DKIM signing with key rotation
  - SPF, DMARC, ARC validation
  - MTA-STS and TLS-RPT
  - IP reputation management

### LMTP Gateway
- **Purpose**: Local mail delivery and blob storage
- **Technology**: Rust, async/await
- **Key Features**:
  - Quota enforcement
  - Content-addressed storage (SHA-256)
  - Zstd compression
  - Deduplication
  - Enqueue indexing jobs

### JMAP Service
- **Purpose**: Modern email API over HTTP/3
- **Technology**: Rust, Stalwart JMAP, QUIC
- **Key Features**:
  - JMAP Core, Mail, Contacts
  - IMAP/Submission fallback
  - State-based synchronization
  - Delta updates
  - Push notifications

### Delivery Worker
- **Purpose**: Finalize mailbox mutations
- **Key Features**:
  - Message deduplication
  - Thread detection
  - Label application
  - Flag updates
  - ModSeq management

### Indexer
- **Purpose**: Full-text search indexing
- **Technology**: Rust, Tantivy
- **Key Features**:
  - Per-tenant shards
  - Text extraction (PDF, Office)
  - Header indexing
  - Optional vector embeddings
  - Real-time updates

### Search Gateway
- **Purpose**: Query planning and execution
- **Key Features**:
  - Hybrid search (lexical + vector)
  - Complex filters (from/to/subject/date/size)
  - Boolean queries
  - Pagination
  - Result ranking

### Previewer
- **Purpose**: Attachment preview generation
- **Key Features**:
  - Image thumbnails (WebP)
  - PDF page previews
  - Office document previews
  - Async job processing

### AI Assistant
- **Purpose**: Email composition and analysis
- **Technology**: Quantized 7-8B model
- **Key Features**:
  - Draft composition
  - Tone rewriting
  - Thread summarization
  - Action extraction
  - Privacy-preserving (no storage)

### Link Service
- **Purpose**: Large attachment handling
- **Key Features**:
  - Signed URLs
  - Time-limited access
  - Download tracking
  - Revocation

### Admin API
- **Purpose**: Platform administration
- **Key Features**:
  - Tenant management
  - Domain verification
  - DKIM rotation
  - User management
  - Billing integration (Stripe)
  - Import/export

### AntiAbuse
- **Purpose**: Security and rate limiting
- **Key Features**:
  - Anomaly detection
  - Rate limiting (per user/IP/ASN)
  - Signup validation
  - Blocklists/allowlists

## Data Stores

### PostgreSQL
- **Purpose**: Metadata, users, messages
- **Features**:
  - Partitioned by tenant and month
  - Read replicas in production
  - Connection pooling
  - Triggers for updated_at

### Redis
- **Purpose**: Caching, job queues, rate limiting
- **Features**:
  - Streams for job queues
  - TTL for caching
  - Lua scripts for atomicity

### Object Storage (R2/MinIO)
- **Purpose**: Email blobs, attachments, previews
- **Features**:
  - Content-addressed (SHA-256)
  - Zstd compression
  - Lifecycle policies
  - Signed URLs

### Tantivy
- **Purpose**: Full-text search indices
- **Features**:
  - Per-tenant sharding
  - SSD-optimized
  - Fast indexing
  - Real-time updates

## Network Architecture

### Public Subnets
- Load balancers
- Edge MTA VMs (static IPs)
- NAT gateways

### Private Subnets
- Kubernetes worker nodes
- Databases (RDS PostgreSQL)
- Redis
- MinIO (dev) / R2 endpoint

## Security

### Authentication
- Passkeys (WebAuthn) preferred
- TOTP fallback
- Per-device tokens
- OAuth for organizations

### Encryption
- TLS 1.3 in transit
- KMS envelope encryption at rest
- Per-tenant data keys
- DKIM key rotation

### Network
- VPC with private subnets
- Security groups
- Network policies
- DDoS protection (Cloudflare)

## Observability

### Metrics
- Prometheus scraping
- RED metrics (Rate, Errors, Duration)
- Custom business metrics

### Traces
- OpenTelemetry
- Distributed tracing
- Span attributes

### Logs
- Structured JSON logging
- Loki aggregation
- Log levels

### Dashboards
- Grafana
- Service health
- Performance metrics
- Business KPIs

## Scaling

### Horizontal
- Kubernetes HPA (CPU + custom metrics)
- Stateless services
- Database connection pooling
- Read replicas

### Vertical
- Resource requests/limits
- RDS instance sizing
- Redis memory configuration

### Partitioning
- Per-tenant Tantivy shards
- PostgreSQL table partitioning (by month)
- Content-addressed blob storage

## SLOs

- **JMAP API p99**: < 200ms
- **Receive to searchable**: < 60s (p95)
- **Webmail first paint**: < 1.2s
- **Uptime**: 99.9%

## Cost Optimization

- Spot instances for workers
- S3/R2 lifecycle policies
- Connection pooling
- Efficient compression (zstd)
- Resource limits
- Auto-scaling policies
