# Incident Response Runbook

## Severity Levels

| Level | Description | Response Time | Examples |
|-------|-------------|---------------|----------|
| **P0** | Critical outage | Immediate | Total mail service down, data loss |
| **P1** | Major degradation | < 15 min | High error rates, slow responses |
| **P2** | Minor issues | < 1 hour | Single service degraded, non-critical feature down |
| **P3** | Low impact | < 4 hours | Cosmetic issues, minor bugs |

## P0: Critical Outage

### Immediate Actions (First 5 minutes)

1. **Acknowledge the incident**
   ```bash
   # Page on-call engineer
   # Post in #incidents Slack channel
   ```

2. **Assess scope**
   ```bash
   # Check service status
   kubectl get pods -n nova-mail

   # Check edge-mta status
   ssh edge-mta-1 "systemctl status stalwart-mail"
   ssh edge-mta-2 "systemctl status stalwart-mail"

   # Check database
   psql $DATABASE_URL -c "SELECT 1;"

   # Check Redis
   redis-cli -h $REDIS_HOST ping
   ```

3. **Check dashboards**
   - Grafana: https://grafana.nova-mail.com/d/overview
   - Prometheus alerts: https://prometheus.nova-mail.com/alerts
   - Error tracking: Sentry dashboard

### Common P0 Scenarios

#### Scenario: All SMTP services down

**Symptoms:**
- No emails being received
- SMTP port 25 not responding
- MX records unreachable

**Diagnosis:**
```bash
# Check if edge-mta servers are up
ping mail1.yourdomain.com
ping mail2.yourdomain.com

# Check SMTP ports
telnet mail1.yourdomain.com 25

# Check Stalwart status
ssh edge-mta-1 "systemctl status stalwart-mail"
ssh edge-mta-1 "journalctl -u stalwart-mail -n 100"
```

**Resolution:**
```bash
# Restart Stalwart if crashed
ssh edge-mta-1 "sudo systemctl restart stalwart-mail"

# Check queue for backed up messages
ssh edge-mta-1 "sudo stalwart-cli queue list"

# If DNS issue, verify MX records
dig +short MX yourdomain.com

# If IP blocked, check with:
# - Spamhaus: https://www.spamhaus.org/lookup/
# - Barracuda: https://www.barracudacentral.org/lookups
```

#### Scenario: Database down

**Symptoms:**
- 500 errors from all services
- "connection refused" in logs
- Cannot authenticate users

**Diagnosis:**
```bash
# Check RDS status
aws rds describe-db-instances --db-instance-identifier nova-mail-prod

# Try connecting
psql $DATABASE_URL -c "SELECT 1;"

# Check connection pool exhaustion
# Look for "too many connections" in logs
kubectl logs -n nova-mail deployment/admin-api | grep -i "connection"
```

**Resolution:**
```bash
# If connection pool exhausted
# Scale down services temporarily
kubectl scale deployment/admin-api --replicas=1 -n nova-mail

# If RDS down, initiate failover
aws rds reboot-db-instance --db-instance-identifier nova-mail-prod

# Or promote read replica
aws rds promote-read-replica --db-instance-identifier nova-mail-prod-replica

# Update connection strings after failover
kubectl set env deployment/admin-api DATABASE_URL=<new-endpoint> -n nova-mail
```

#### Scenario: Kubernetes cluster down

**Symptoms:**
- kubectl commands failing
- All web services unreachable
- JMAP API down

**Diagnosis:**
```bash
# Check EKS cluster status
aws eks describe-cluster --name nova-mail-prod

# Check node status
kubectl get nodes

# Check control plane logs
aws eks describe-cluster --name nova-mail-prod | jq '.cluster.logging'
```

**Resolution:**
```bash
# If nodes unhealthy, cordon and drain
kubectl cordon <node-name>
kubectl drain <node-name> --ignore-daemonsets --delete-emptydir-data

# Terminate node and let ASG replace
aws ec2 terminate-instances --instance-ids <instance-id>

# If control plane unreachable, contact AWS Support
# Fallback: Edge-mta still operating for incoming mail
```

## P1: Major Degradation

### Examples

- High latency (p99 > 2s)
- Elevated error rates (> 5%)
- Search service down
- Single availability zone failure

### Response

1. **Assess impact**
   - How many users affected?
   - Which features degraded?
   - Is data at risk?

2. **Mitigate**
   - Scale up if capacity issue
   - Restart unhealthy pods
   - Route traffic away from degraded zone

3. **Investigate root cause**
   - Check recent deployments
   - Review metrics and traces
   - Examine error logs

### Example: High Database Latency

```bash
# Check slow queries
SELECT pid, now() - pg_stat_activity.query_start AS duration, query
FROM pg_stat_activity
WHERE (now() - pg_stat_activity.query_start) > interval '5 seconds'
ORDER BY duration DESC;

# Check for long-running transactions
SELECT pid, now() - xact_start AS duration
FROM pg_stat_activity
WHERE state = 'idle in transaction'
ORDER BY duration DESC;

# Kill long-running query if needed
SELECT pg_terminate_backend(<pid>);

# Check RDS metrics
aws cloudwatch get-metric-statistics \
  --namespace AWS/RDS \
  --metric-name DatabaseConnections \
  --dimensions Name=DBInstanceIdentifier,Value=nova-mail-prod \
  --start-time 2025-01-01T00:00:00Z \
  --end-time 2025-01-01T01:00:00Z \
  --period 300 \
  --statistics Average
```

## Communication

### Status Page Updates

```bash
# Update status page (manual or via API)
curl -X POST https://status.nova-mail.com/api/incidents \
  -H "Authorization: Bearer $STATUS_PAGE_TOKEN" \
  -d '{
    "name": "Degraded JMAP API Performance",
    "status": "investigating",
    "message": "We are investigating reports of slow email loading times.",
    "component_id": "jmap-api"
  }'
```

### User Communication Template

**Subject:** [Service Status] Investigating Email Delivery Issues

```
We are currently investigating reports of delayed email delivery.

Status: Investigating
Impact: Some emails may be delayed by up to 15 minutes
Started: 2025-01-08 10:30 UTC

We will provide updates every 30 minutes until resolved.

Next update: 11:00 UTC
```

## Post-Incident

### Incident Report Template

1. **Summary**
   - What happened?
   - When did it start/end?
   - Who was affected?

2. **Timeline**
   - 10:30 - First alert received
   - 10:32 - On-call engineer paged
   - 10:35 - Root cause identified
   - 10:45 - Fix deployed
   - 11:00 - Service restored

3. **Root Cause**
   - What caused the incident?
   - Why didn't we catch it earlier?

4. **Resolution**
   - What fixed the issue?
   - Temporary vs permanent fix?

5. **Action Items**
   - [ ] Improve monitoring
   - [ ] Add automated remediation
   - [ ] Update runbook
   - [ ] Conduct blameless postmortem

## Escalation

### When to Escalate

- P0 not resolved in 1 hour
- P1 not resolved in 4 hours
- Data loss suspected
- Security incident
- Multiple services failing simultaneously

### Escalation Contacts

1. **Engineering Lead**: [name@nova-mail.com]
2. **CTO**: [cto@nova-mail.com]
3. **AWS Support**: Premium support case
4. **Cloudflare Support**: Enterprise support ticket

## Useful Commands

```bash
# Quick health check
make health-check

# Check recent deployments
kubectl rollout history deployment/admin-api -n nova-mail

# Rollback deployment
kubectl rollout undo deployment/admin-api -n nova-mail

# Get logs from all replicas
kubectl logs -l app=admin-api -n nova-mail --tail=100

# Check resource usage
kubectl top pods -n nova-mail

# Exec into pod for debugging
kubectl exec -it deployment/admin-api -n nova-mail -- /bin/sh

# Port-forward to debug locally
kubectl port-forward svc/admin-api 8080:80 -n nova-mail
```

## Related Documents

- [On-Call Procedures](./on-call.md)
- [Deployment Runbook](./deployment.md)
- [Database Maintenance](./database-maintenance.md)
- [Security Incident Response](./security-incident.md)
