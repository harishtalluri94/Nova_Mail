#!/bin/bash
set -euo pipefail

# Generate DKIM key pair for a domain

DOMAIN=${1:-example.com}
SELECTOR=${2:-s1}
KEY_SIZE=2048

echo "Generating DKIM key pair for domain: $DOMAIN"
echo "Selector: $SELECTOR"

# Create output directory
OUTPUT_DIR="./dkim-keys/$DOMAIN"
mkdir -p "$OUTPUT_DIR"

# Generate private key
openssl genrsa -out "$OUTPUT_DIR/$SELECTOR.private.pem" $KEY_SIZE

# Extract public key
openssl rsa -in "$OUTPUT_DIR/$SELECTOR.private.pem" -pubout -out "$OUTPUT_DIR/$SELECTOR.public.pem"

# Generate DNS TXT record value
PUBLIC_KEY=$(grep -v "BEGIN\|END" "$OUTPUT_DIR/$SELECTOR.public.pem" | tr -d '\n')
DNS_RECORD="${SELECTOR}._domainkey.${DOMAIN} IN TXT \"v=DKIM1; k=rsa; p=${PUBLIC_KEY}\""

echo ""
echo "✓ DKIM keys generated successfully!"
echo ""
echo "Private key: $OUTPUT_DIR/$SELECTOR.private.pem"
echo "Public key:  $OUTPUT_DIR/$SELECTOR.public.pem"
echo ""
echo "DNS TXT Record:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "$DNS_RECORD"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Add this record to your DNS zone to enable DKIM signing."
