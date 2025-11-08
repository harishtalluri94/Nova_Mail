# Nova Mail Deployment Guide

## Prerequisites

- AWS account with appropriate permissions
- Terraform >= 1.5
- kubectl >= 1.28
- Helm >= 3.12
- Docker >= 24.0
- kind >= 0.20 (for local development)

## Local Development

### 1. Start Development Environment

```bash
# Start local services (PostgreSQL, Redis, MinIO, etc.)
make dev-up
```

This will:
- Create a kind cluster named `nova-mail`
- Start PostgreSQL, Redis, MinIO via docker-compose
- Start observability stack (Prometheus, Grafana, Loki)

### 2. Build Services

```bash
# Build all Rust services
make build

# Build Docker images
make docker-build
```

### 3. Deploy to Local Cluster

```bash
# Deploy all services to kind
make deploy-local
```

### 4. Access Services

- **Webmail**: http://localhost:3000
- **Admin API**: http://localhost:8080
- **Grafana**: http://localhost:3001 (admin/admin)
- **Prometheus**: http://localhost:9090
- **MinIO Console**: http://localhost:9001 (minioadmin/minioadmin)

### 5. Run Demo

```bash
# Seed demo data and start services
make demo
```

Test credentials:
- Email: demo@nova.local
- Password: demo123

## Staging Deployment

### 1. Initialize Terraform

```bash
cd deploy/terraform/envs/stage
terraform init
```

### 2. Configure Variables

Create `terraform.tfvars`:

```hcl
environment = "stage"
region      = "us-west-2"

# VPC
availability_zones = ["us-west-2a", "us-west-2b"]

# RDS
db_instance_class = "db.t4g.small"
db_allocated_storage = 100

# EKS
eks_cluster_version = "1.28"
eks_node_instance_types = ["t3.medium"]
eks_node_desired_size = 3
eks_node_min_size = 2
eks_node_max_size = 5

# Domain
domain_name = "stage.nova-mail.com"
```

### 3. Deploy Infrastructure

```bash
terraform plan
terraform apply
```

This creates:
- VPC with public/private subnets
- EKS cluster
- RDS PostgreSQL instance
- ElastiCache Redis
- 2 EC2 instances for edge-mta
- S3 buckets (or R2 if using Cloudflare)

### 4. Configure kubectl

```bash
aws eks update-kubeconfig --region us-west-2 --name nova-mail-stage
```

### 5. Deploy Edge MTA

SSH into edge-mta VMs:

```bash
ssh -i edge-mta-key.pem ec2-user@<edge-mta-ip>
```

Install Stalwart:

```bash
# Follow Stalwart installation guide
# Copy configuration from servers/edge-mta/config/
# Set up DKIM keys
# Configure Rspamd
```

### 6. Deploy Kubernetes Services

```bash
cd deploy/k8s

# Create namespace
kubectl create namespace nova-mail

# Create secrets
kubectl create secret generic admin-api-secrets \
  --from-literal=database-url="postgres://..." \
  --from-literal=redis-url="redis://..." \
  -n nova-mail

# Deploy with Helm
helm install admin-api base/admin-api -n nova-mail
helm install lmtp-gateway base/lmtp-gateway -n nova-mail
# ... repeat for all services
```

### 7. Configure DNS

Add DNS records:

```
# A records for edge-mta
mail1.nova-mail.com.  A  <edge-mta-1-ip>
mail2.nova-mail.com.  A  <edge-mta-2-ip>

# MX record
nova-mail.com.  MX  10 mail1.nova-mail.com.
nova-mail.com.  MX  20 mail2.nova-mail.com.

# DKIM (get from `make gen-dkim`)
s1._domainkey.nova-mail.com.  TXT  "v=DKIM1; k=rsa; p=..."

# SPF
nova-mail.com.  TXT  "v=spf1 include:_spf.nova-mail.com ~all"

# DMARC
_dmarc.nova-mail.com.  TXT  "v=DMARC1; p=quarantine; rua=mailto:dmarc@nova-mail.com"

# MTA-STS
_mta-sts.nova-mail.com.  TXT  "v=STSv1; id=20250101"

# Web services (via ALB/Cloudflare)
api.nova-mail.com.  CNAME  <alb-dns-name>
mail.nova-mail.com.  CNAME  <cloudflare-proxy>
```

### 8. Verify Deployment

```bash
# Check pod status
kubectl get pods -n nova-mail

# Check logs
kubectl logs -f deployment/admin-api -n nova-mail

# Run health checks
curl https://api.nova-mail.com/healthz
curl https://api.nova-mail.com/readyz

# Test SMTP
telnet mail1.nova-mail.com 25
```

## Production Deployment

### Differences from Staging

1. **High Availability**:
   - Multi-AZ RDS with read replica
   - Multiple EKS node groups across AZs
   - Redis cluster mode
   - Multiple edge-mta VMs per AZ

2. **Scaling**:
   - Larger instance sizes
   - Higher HPA limits
   - Increased connection pools

3. **Security**:
   - VPC peering for cross-region DR
   - AWS KMS for encryption
   - WAF rules
   - Network policies enforced

4. **Monitoring**:
   - PagerDuty integration
   - Enhanced alerting rules
   - 24/7 on-call rotation

### Production Checklist

- [ ] DNS records configured and verified
- [ ] TLS certificates issued
- [ ] DKIM keys generated and in DNS
- [ ] SPF, DMARC records set
- [ ] MTA-STS policy published
- [ ] IP reputation established (warm-up)
- [ ] Database backups configured
- [ ] Disaster recovery plan tested
- [ ] Monitoring alerts configured
- [ ] Runbooks documented
- [ ] Security audit completed
- [ ] Load testing passed
- [ ] Incident response plan ready

### Rolling Updates

```bash
# Update image tag
helm upgrade admin-api base/admin-api \
  --set image.tag=v1.2.3 \
  -n nova-mail

# Watch rollout
kubectl rollout status deployment/admin-api -n nova-mail

# Rollback if needed
kubectl rollout undo deployment/admin-api -n nova-mail
```

### Database Migrations

```bash
# Run migrations via job
kubectl apply -f deploy/k8s/jobs/migrate.yaml

# Check migration status
kubectl logs job/migrate -n nova-mail
```

### Scaling

```bash
# Scale horizontally
kubectl scale deployment/admin-api --replicas=5 -n nova-mail

# Scale vertically (update Helm values)
helm upgrade admin-api base/admin-api \
  --set resources.requests.cpu=1000m \
  --set resources.requests.memory=2Gi \
  -n nova-mail
```

## Troubleshooting

### Pods Not Starting

```bash
# Check events
kubectl describe pod <pod-name> -n nova-mail

# Check logs
kubectl logs <pod-name> -n nova-mail

# Check resource limits
kubectl top pods -n nova-mail
```

### Database Connection Issues

```bash
# Test from pod
kubectl exec -it <pod-name> -n nova-mail -- /bin/sh
# Inside pod:
psql $DATABASE_URL
```

### Email Delivery Issues

```bash
# Check edge-mta logs
ssh edge-mta-1
sudo journalctl -u stalwart -f

# Check rspamd
sudo rspamc stat

# Check queue
sudo stalwart-cli queue list
```

### Performance Issues

```bash
# Check metrics
kubectl port-forward svc/prometheus 9090:9090 -n observability
# Open http://localhost:9090

# Check traces
kubectl port-forward svc/tempo 3200:3200 -n observability
```

## Backup and Restore

### Database Backup

```bash
# Manual backup
pg_dump -h <rds-endpoint> -U nova_admin -d nova_mail > backup.sql

# Restore
psql -h <rds-endpoint> -U nova_admin -d nova_mail < backup.sql
```

### Object Storage Backup

```bash
# S3/R2 versioning enabled by default
# Point-in-time recovery via AWS/Cloudflare console
```

## Monitoring and Alerts

### Key Metrics

- JMAP request latency (p50, p95, p99)
- SMTP queue depth
- Database connection pool usage
- Redis hit rate
- Object storage throughput
- Search index lag

### Critical Alerts

- Service down (> 3 pods unavailable)
- High error rate (> 1% 5xx)
- Database connection pool exhausted
- Disk space low (< 20%)
- Certificate expiring (< 7 days)

## Security Best Practices

1. Rotate secrets regularly
2. Enable audit logging
3. Use least-privilege IAM roles
4. Enable VPC flow logs
5. Regular security scans (Trivy, Grype)
6. Keep dependencies updated
7. Monitor CVE databases

## Cost Optimization

1. Use spot instances for non-critical workloads
2. Enable S3/R2 lifecycle policies
3. Right-size RDS instances
4. Use Redis cluster mode sparingly
5. Monitor and alert on cost anomalies
6. Clean up unused resources

## Support

- Documentation: `/ops/docs/`
- Runbooks: `/ops/docs/runbooks/`
- Issues: GitHub Issues
- On-call: PagerDuty rotation
