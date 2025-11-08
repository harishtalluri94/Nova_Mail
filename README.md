# Nova Mail

A FAANG-grade, high-performance email platform optimized for low cost and exceptional user experience.

## Overview

Nova Mail is a modern email platform designed with:
- **Performance**: p99 latency < 200ms for JMAP operations
- **Cost Efficiency**: Low hundreds USD/month for private beta (≤10k users)
- **Security**: Passkeys, KMS encryption, comprehensive email authentication
- **Interoperability**: Full SPF, DKIM, DMARC, ARC, MTA-STS, TLS-RPT support
- **Modern Stack**: Rust services, PostgreSQL, Tantivy search, HTTP/3, Next.js web

## Architecture

### Backend Services (12)

1. **edge-mta** - SMTP ingress/egress with DKIM signing
2. **lmtp-gateway** - Local mail delivery and quota enforcement
3. **jmap-service** - JMAP/IMAP/Submission server (HTTP/3)
4. **delivery-worker** - Mailbox mutation finalization
5. **indexer** - Full-text search indexing with Tantivy
6. **search-gateway** - Hybrid search (lexical + optional vector)
7. **previewer** - Attachment thumbnail generation
8. **ai-assistant** - Email composition and summarization
9. **link-service** - Time-limited attachment links
10. **admin-api** - Tenant and domain management
11. **antiabuse** - Anomaly detection and rate limiting
12. **observability** - Metrics, traces, and logs

### Frontend

- **webmail** - Next.js + TipTap rich editor

### Data Stores

- **PostgreSQL 15+** - Metadata, users, messages
- **Cloudflare R2 / MinIO** - Blob storage (emails, attachments)
- **Tantivy** - Full-text search indices
- **Redis** - Caching, job queues, rate limiting

## Repository Structure

```
/Nova_Mail
  /servers          # Stateful mail servers (VMs)
    /edge-mta       # Stalwart SMTP configuration
  /services         # Stateless K8s services
    /lmtp-gateway
    /jmap-service
    /delivery-worker
    /indexer
    /search-gateway
    /previewer
    /ai-assistant
    /link-service
    /admin-api
    /antiabuse
    /observability
  /web              # Frontend applications
    /webmail
  /libs             # Shared libraries
    /jmap-sdk
    /mime-tools
    /common
  /deploy           # Infrastructure as Code
    /k8s            # Helm charts
    /terraform      # Cloud resources
  /ops              # Operations
    /docs           # Runbooks, playbooks
    /scripts        # Utilities
  /tests            # E2E and integration tests
```

## Quick Start

### Prerequisites

- Docker & Docker Compose
- Kubernetes (kind for local dev)
- Terraform >= 1.5
- Rust >= 1.75
- Node.js >= 20

### Local Development

```bash
# Start local development environment
make dev-up

# Run tests
make test

# Build all services
make build

# Deploy to kind cluster
make deploy-local
```

### Production Deployment

```bash
# Initialize infrastructure
cd deploy/terraform/envs/prod
terraform init
terraform apply

# Deploy services
cd deploy/k8s
helmfile apply
```

## Non-Functional Requirements

- **SLOs**: JMAP p99 < 200ms, receive-to-searchable < 60s
- **Uptime**: 99.9% target
- **Security**: Passkeys, KMS envelope encryption, TLS 1.3
- **Email Auth**: SPF, DKIM, DMARC, ARC, MTA-STS, TLS-RPT
- **Observability**: OpenTelemetry, Prometheus, Grafana, Loki

## Development Roadmap

- [x] Phase 0: Repository scaffold
- [ ] Phase 1: Infrastructure (VPC, EKS, RDS, Redis)
- [ ] Phase 2: Mail plane (SMTP, LMTP, rspamd)
- [ ] Phase 3: Data plane (PostgreSQL schema, object storage)
- [ ] Phase 4: JMAP/IMAP & webmail MVP
- [ ] Phase 5: Indexing & search
- [ ] Phase 6: Previews & large files
- [ ] Phase 7: AI assistant
- [ ] Phase 8: Admin & billing
- [ ] Phase 9: Observability & SLOs
- [ ] Phase 10: Hardening & launch prep

## Documentation

- [Architecture Overview](ops/docs/architecture.md)
- [Deployment Guide](ops/docs/deployment.md)
- [Development Guide](ops/docs/development.md)
- [API Documentation](ops/docs/api.md)
- [Runbooks](ops/docs/runbooks/)
- [SLO Definitions](ops/docs/slos.md)

## License

Proprietary - All Rights Reserved

## Contributing

This is a private project. Contact the team for access.
