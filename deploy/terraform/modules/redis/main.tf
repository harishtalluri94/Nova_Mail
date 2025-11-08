terraform {
  required_version = ">= 1.5"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "environment" {
  description = "Environment name"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "subnet_ids" {
  description = "List of subnet IDs for Redis"
  type        = list(string)
}

variable "node_type" {
  description = "ElastiCache node type"
  type        = string
  default     = "cache.t4g.small"
}

variable "num_cache_nodes" {
  description = "Number of cache nodes"
  type        = number
  default     = 1
}

variable "engine_version" {
  description = "Redis engine version"
  type        = string
  default     = "7.0"
}

locals {
  common_tags = {
    Project     = "nova-mail"
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

# Security Group
resource "aws_security_group" "redis" {
  name        = "nova-mail-${var.environment}-redis"
  description = "Security group for ElastiCache Redis"
  vpc_id      = var.vpc_id

  ingress {
    from_port   = 6379
    to_port     = 6379
    protocol    = "tcp"
    cidr_blocks = ["10.0.0.0/8"]
    description = "Redis from VPC"
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}-redis"
  })
}

# Subnet Group
resource "aws_elasticache_subnet_group" "main" {
  name       = "nova-mail-${var.environment}"
  subnet_ids = var.subnet_ids

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}"
  })
}

# Parameter Group
resource "aws_elasticache_parameter_group" "main" {
  name   = "nova-mail-${var.environment}-redis7"
  family = "redis7"

  parameter {
    name  = "maxmemory-policy"
    value = "allkeys-lru"
  }

  parameter {
    name  = "timeout"
    value = "300"
  }

  parameter {
    name  = "tcp-keepalive"
    value = "300"
  }

  tags = local.common_tags
}

# Redis Cluster (single-node for dev/stage, multi-node for prod)
resource "aws_elasticache_replication_group" "main" {
  replication_group_id = "nova-mail-${var.environment}"
  description          = "Nova Mail Redis cluster"

  engine               = "redis"
  engine_version       = var.engine_version
  node_type            = var.node_type
  port                 = 6379
  parameter_group_name = aws_elasticache_parameter_group.main.name

  # Clustering
  num_cache_clusters         = var.num_cache_nodes
  automatic_failover_enabled = var.num_cache_nodes > 1
  multi_az_enabled           = var.num_cache_nodes > 1

  # Networking
  subnet_group_name  = aws_elasticache_subnet_group.main.name
  security_group_ids = [aws_security_group.redis.id]

  # Maintenance
  maintenance_window         = "sun:05:00-sun:06:00"
  snapshot_window            = "03:00-04:00"
  snapshot_retention_limit   = var.environment == "prod" ? 7 : 1
  final_snapshot_identifier  = var.environment == "prod" ? "nova-mail-${var.environment}-final" : null
  auto_minor_version_upgrade = true

  # Encryption
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token_enabled         = true
  auth_token                 = random_password.redis_auth.result

  # Logging
  log_delivery_configuration {
    destination      = aws_cloudwatch_log_group.redis_slow_log.name
    destination_type = "cloudwatch-logs"
    log_format       = "json"
    log_type         = "slow-log"
  }

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}"
  })
}

# Auth token
resource "random_password" "redis_auth" {
  length  = 32
  special = true
}

# Store auth token in Secrets Manager
resource "aws_secretsmanager_secret" "redis_auth" {
  name = "nova-mail-${var.environment}-redis-auth"
  tags = local.common_tags
}

resource "aws_secretsmanager_secret_version" "redis_auth" {
  secret_id     = aws_secretsmanager_secret.redis_auth.id
  secret_string = random_password.redis_auth.result
}

# CloudWatch Log Group for slow log
resource "aws_cloudwatch_log_group" "redis_slow_log" {
  name              = "/aws/elasticache/nova-mail-${var.environment}/slow-log"
  retention_in_days = 7

  tags = local.common_tags
}

# Outputs
output "endpoint" {
  description = "Redis primary endpoint"
  value       = aws_elasticache_replication_group.main.primary_endpoint_address
}

output "reader_endpoint" {
  description = "Redis reader endpoint"
  value       = aws_elasticache_replication_group.main.reader_endpoint_address
}

output "port" {
  description = "Redis port"
  value       = aws_elasticache_replication_group.main.port
}

output "auth_secret_arn" {
  description = "ARN of secret containing Redis auth token"
  value       = aws_secretsmanager_secret.redis_auth.arn
}

output "connection_string" {
  description = "Redis connection string"
  value       = "rediss://${aws_elasticache_replication_group.main.primary_endpoint_address}:${aws_elasticache_replication_group.main.port}"
  sensitive   = true
}
