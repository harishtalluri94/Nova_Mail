# DKIM Key Rotation Runbook

## Overview

DKIM (DomainKeys Identified Mail) keys should be rotated periodically (every 6-12 months) to maintain security.

## Prerequisites

- Access to DNS management console
- SSH access to edge-mta servers
- Admin API credentials

## Procedure

### 1. Generate New DKIM Key Pair

```bash
# From ops directory
./scripts/gen-dkim-key.sh yourdomain.com s2

# This generates:
# - dkim-keys/yourdomain.com/s2.private.pem
# - dkim-keys/yourdomain.com/s2.public.pem
# - DNS TXT record value
```

### 2. Add New DNS Record

Add the new DKIM DNS record **before** switching selectors:

```
s2._domainkey.yourdomain.com. IN TXT "v=DKIM1; k=rsa; p=<public-key>"
```

Wait for DNS propagation (check with `dig`):

```bash
dig +short TXT s2._domainkey.yourdomain.com
```

### 3. Update Stalwart Configuration

On each edge-mta server:

```bash
# Copy new private key
sudo cp dkim-keys/yourdomain.com/s2.private.pem /etc/stalwart/dkim/s2.key
sudo chown stalwart:stalwart /etc/stalwart/dkim/s2.key
sudo chmod 600 /etc/stalwart/dkim/s2.key

# Update Stalwart config to use new selector
sudo nano /etc/stalwart/config.toml

# Change selector line:
# selector = "s2"

# Test configuration
sudo stalwart-cli config test

# Reload (graceful, no downtime)
sudo systemctl reload stalwart-mail
```

### 4. Update Database

Update domain record in database:

```sql
UPDATE domains
SET dkim_selector = 's2',
    dkim_private_key = '<encrypted-new-key>',
    dkim_public_key = '<new-public-key>',
    updated_at = NOW()
WHERE domain = 'yourdomain.com';
```

### 5. Verify DKIM Signing

Send a test email and verify DKIM signature:

```bash
# Send test email
echo "Test email" | mail -s "DKIM Test" test@mail-tester.com

# Check mail-tester.com results or use:
swaks --to test@yourdomain.com \
      --from sender@yourdomain.com \
      --server mail1.yourdomain.com \
      --tls
```

Verify DKIM header in received email:
```
DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed;
    d=yourdomain.com; s=s2; ...
```

### 6. Monitor for 7 Days

Monitor DMARC reports for any DKIM failures:

```bash
# Check DMARC reports
sudo grep "dkim=fail" /var/log/stalwart/mail.log

# Or query database
SELECT COUNT(*) FROM dmarc_reports
WHERE domain_id = '<domain-id>'
  AND report_xml LIKE '%dkim=fail%'
  AND begin_date > NOW() - INTERVAL '7 days';
```

### 7. Remove Old DNS Record

After 7 days with no issues, remove the old DNS record:

```bash
# Delete old record
s1._domainkey.yourdomain.com. IN TXT
```

### 8. Clean Up Old Keys

```bash
# On edge-mta servers
sudo rm /etc/stalwart/dkim/s1.key

# From ops directory
rm -f dkim-keys/yourdomain.com/s1.*.pem
```

## Rollback Procedure

If issues occur:

1. Revert Stalwart config to old selector (s1)
2. Reload Stalwart: `sudo systemctl reload stalwart-mail`
3. Revert database changes
4. Investigate issues before retrying

## Verification Checklist

- [ ] New DKIM DNS record added and propagated
- [ ] New private key deployed to all edge-mta servers
- [ ] Stalwart configuration updated
- [ ] Database updated
- [ ] Test email sent with valid DKIM signature
- [ ] Selector in DKIM-Signature header matches new selector
- [ ] No DKIM failures in logs for 7 days
- [ ] Old DNS record removed
- [ ] Old private keys deleted

## Troubleshooting

### DNS not propagating

```bash
# Check DNS from different servers
dig @8.8.8.8 +short TXT s2._domainkey.yourdomain.com
dig @1.1.1.1 +short TXT s2._domainkey.yourdomain.com

# Check TTL
dig +noall +answer TXT s2._domainkey.yourdomain.com
```

### DKIM signature failing

```bash
# Check Stalwart logs
sudo journalctl -u stalwart-mail -f | grep -i dkim

# Verify key permissions
ls -la /etc/stalwart/dkim/

# Test DKIM signing manually
sudo stalwart-cli dkim sign --selector s2 --domain yourdomain.com
```

### Multiple selectors causing issues

Ensure only ONE selector is active in Stalwart config. Multiple selectors can cause confusion.

## Automation

Consider automating DKIM rotation:

```bash
# Add to crontab for monthly rotation check
0 0 1 * * /opt/nova-mail/scripts/check-dkim-age.sh
```

## Related Documents

- [DKIM Specification (RFC 6376)](https://www.rfc-editor.org/rfc/rfc6376)
- [Stalwart DKIM Configuration](https://stalw.art/docs/smtp/authentication/dkim)
- DNS Management Runbook
- Email Deliverability Playbook
