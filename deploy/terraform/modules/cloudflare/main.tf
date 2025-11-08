terraform {
  required_version = ">= 1.5"
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }
}

variable "environment" {
  description = "Environment name"
  type        = string
}

variable "domain_name" {
  description = "Root domain name"
  type        = string
}

variable "zone_id" {
  description = "Cloudflare Zone ID"
  type        = string
}

variable "edge_mta_ips" {
  description = "List of edge MTA IP addresses"
  type        = list(string)
  default     = []
}

locals {
  common_tags = {
    Project     = "nova-mail"
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

# R2 Buckets
resource "cloudflare_r2_bucket" "mail_blobs" {
  account_id = var.account_id
  name       = "nova-mail-${var.environment}-blobs"
  location   = "auto"
}

resource "cloudflare_r2_bucket" "previews" {
  account_id = var.account_id
  name       = "nova-mail-${var.environment}-previews"
  location   = "auto"
}

resource "cloudflare_r2_bucket" "exports" {
  account_id = var.account_id
  name       = "nova-mail-${var.environment}-exports"
  location   = "auto"
}

variable "account_id" {
  description = "Cloudflare Account ID"
  type        = string
}

# DNS Records for Mail
resource "cloudflare_record" "mx" {
  count = length(var.edge_mta_ips)

  zone_id  = var.zone_id
  name     = var.domain_name
  type     = "MX"
  priority = (count.index + 1) * 10
  value    = "mail${count.index + 1}.${var.domain_name}"
  ttl      = 3600
}

resource "cloudflare_record" "mail_a" {
  count = length(var.edge_mta_ips)

  zone_id = var.zone_id
  name    = "mail${count.index + 1}"
  type    = "A"
  value   = var.edge_mta_ips[count.index]
  ttl     = 3600
}

# SPF Record
resource "cloudflare_record" "spf" {
  zone_id = var.zone_id
  name    = var.domain_name
  type    = "TXT"
  value   = "v=spf1 include:_spf.${var.domain_name} ~all"
  ttl     = 3600
}

# SPF Include Record
resource "cloudflare_record" "spf_include" {
  zone_id = var.zone_id
  name    = "_spf"
  type    = "TXT"
  value   = "v=spf1 ${join(" ", [for ip in var.edge_mta_ips : "ip4:${ip}"])} ~all"
  ttl     = 3600
}

# DMARC Record
resource "cloudflare_record" "dmarc" {
  zone_id = var.zone_id
  name    = "_dmarc"
  type    = "TXT"
  value   = "v=DMARC1; p=quarantine; rua=mailto:dmarc@${var.domain_name}; ruf=mailto:dmarc@${var.domain_name}; fo=1; adkim=r; aspf=r; pct=100; ri=86400"
  ttl     = 3600
}

# MTA-STS Policy Record
resource "cloudflare_record" "mta_sts" {
  zone_id = var.zone_id
  name    = "_mta-sts"
  type    = "TXT"
  value   = "v=STSv1; id=${formatdate("YYYYMMDD", timestamp())}"
  ttl     = 3600
}

# MTA-STS Host Record
resource "cloudflare_record" "mta_sts_host" {
  zone_id = var.zone_id
  name    = "mta-sts"
  type    = "CNAME"
  value   = var.domain_name
  proxied = true
  ttl     = 1 # Auto when proxied
}

# TLS Reporting Record
resource "cloudflare_record" "tls_report" {
  zone_id = var.zone_id
  name    = "_smtp._tls"
  type    = "TXT"
  value   = "v=TLSRPTv1; rua=mailto:tls-reports@${var.domain_name}"
  ttl     = 3600
}

# Webmail CNAME
resource "cloudflare_record" "webmail" {
  zone_id = var.zone_id
  name    = "mail"
  type    = "CNAME"
  value   = var.domain_name
  proxied = true
  ttl     = 1 # Auto when proxied
}

# API CNAME
resource "cloudflare_record" "api" {
  zone_id = var.zone_id
  name    = "api"
  type    = "CNAME"
  value   = var.domain_name
  proxied = true
  ttl     = 1 # Auto when proxied
}

# Page Rules for HTTP/3
resource "cloudflare_page_rule" "http3_mail" {
  zone_id  = var.zone_id
  target   = "mail.${var.domain_name}/*"
  priority = 1

  actions {
    http3               = "on"
    tls_1_3             = "on"
    automatic_https_rewrites = "on"
    ssl                 = "strict"
  }
}

resource "cloudflare_page_rule" "http3_api" {
  zone_id  = var.zone_id
  target   = "api.${var.domain_name}/*"
  priority = 2

  actions {
    http3               = "on"
    tls_1_3             = "on"
    automatic_https_rewrites = "on"
    ssl                 = "strict"
  }
}

# Outputs
output "r2_bucket_mail_blobs" {
  description = "R2 bucket name for mail blobs"
  value       = cloudflare_r2_bucket.mail_blobs.name
}

output "r2_bucket_previews" {
  description = "R2 bucket name for previews"
  value       = cloudflare_r2_bucket.previews.name
}

output "r2_bucket_exports" {
  description = "R2 bucket name for exports"
  value       = cloudflare_r2_bucket.exports.name
}

output "mail_endpoints" {
  description = "Mail server endpoints"
  value       = [for i in range(length(var.edge_mta_ips)) : "mail${i + 1}.${var.domain_name}"]
}

output "webmail_url" {
  description = "Webmail URL"
  value       = "https://mail.${var.domain_name}"
}

output "api_url" {
  description = "API URL"
  value       = "https://api.${var.domain_name}"
}
