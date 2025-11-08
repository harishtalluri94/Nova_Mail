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
  description = "List of subnet IDs for RDS"
  type        = list(string)
}

variable "instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.t4g.small"
}

variable "allocated_storage" {
  description = "Allocated storage in GB"
  type        = number
  default     = 100
}

variable "database_name" {
  description = "Database name"
  type        = string
  default     = "nova_mail"
}

variable "master_username" {
  description = "Master username"
  type        = string
  default     = "nova_admin"
}

variable "backup_retention_period" {
  description = "Backup retention period in days"
  type        = number
  default     = 7
}

locals {
  common_tags = {
    Project     = "nova-mail"
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

# Generate random password
resource "random_password" "master_password" {
  length  = 32
  special = true
}

# Store password in Secrets Manager
resource "aws_secretsmanager_secret" "db_password" {
  name = "nova-mail-${var.environment}-db-password"

  tags = local.common_tags
}

resource "aws_secretsmanager_secret_version" "db_password" {
  secret_id     = aws_secretsmanager_secret.db_password.id
  secret_string = random_password.master_password.result
}

# DB Subnet Group
resource "aws_db_subnet_group" "main" {
  name       = "nova-mail-${var.environment}"
  subnet_ids = var.subnet_ids

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}"
  })
}

# Security Group
resource "aws_security_group" "rds" {
  name        = "nova-mail-${var.environment}-rds"
  description = "Security group for RDS PostgreSQL"
  vpc_id      = var.vpc_id

  ingress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = ["10.0.0.0/8"] # Adjust based on VPC CIDR
    description = "PostgreSQL from VPC"
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Allow all outbound"
  }

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}-rds"
  })
}

# Parameter Group
resource "aws_db_parameter_group" "main" {
  name   = "nova-mail-${var.environment}-pg15"
  family = "postgres15"

  parameter {
    name  = "max_connections"
    value = "200"
  }

  parameter {
    name  = "shared_buffers"
    value = "{DBInstanceClassMemory/32768}"
  }

  parameter {
    name  = "effective_cache_size"
    value = "{DBInstanceClassMemory/16384}"
  }

  parameter {
    name  = "maintenance_work_mem"
    value = "2097152" # 2GB
  }

  parameter {
    name  = "checkpoint_completion_target"
    value = "0.9"
  }

  parameter {
    name  = "wal_buffers"
    value = "16384" # 16MB
  }

  parameter {
    name  = "default_statistics_target"
    value = "100"
  }

  parameter {
    name  = "random_page_cost"
    value = "1.1" # For SSD
  }

  parameter {
    name  = "effective_io_concurrency"
    value = "200"
  }

  parameter {
    name  = "work_mem"
    value = "10485" # 10MB
  }

  tags = local.common_tags
}

# RDS Instance
resource "aws_db_instance" "main" {
  identifier     = "nova-mail-${var.environment}"
  engine         = "postgres"
  engine_version = "15.4"
  instance_class = var.instance_class

  allocated_storage     = var.allocated_storage
  max_allocated_storage = var.allocated_storage * 2
  storage_type          = "gp3"
  storage_encrypted     = true

  db_name  = var.database_name
  username = var.master_username
  password = random_password.master_password.result

  db_subnet_group_name   = aws_db_subnet_group.main.name
  vpc_security_group_ids = [aws_security_group.rds.id]
  parameter_group_name   = aws_db_parameter_group.main.name

  backup_retention_period = var.backup_retention_period
  backup_window           = "03:00-04:00"
  maintenance_window      = "mon:04:00-mon:05:00"

  enabled_cloudwatch_logs_exports = ["postgresql", "upgrade"]

  multi_az               = var.environment == "prod" ? true : false
  deletion_protection    = var.environment == "prod" ? true : false
  skip_final_snapshot    = var.environment != "prod"
  final_snapshot_identifier = var.environment == "prod" ? "nova-mail-${var.environment}-final-${formatdate("YYYY-MM-DD-hhmm", timestamp())}" : null

  performance_insights_enabled = true

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}"
  })
}

# Read Replica (for prod)
resource "aws_db_instance" "replica" {
  count = var.environment == "prod" ? 1 : 0

  identifier          = "nova-mail-${var.environment}-replica"
  replicate_source_db = aws_db_instance.main.identifier
  instance_class      = var.instance_class

  storage_encrypted = true

  vpc_security_group_ids = [aws_security_group.rds.id]

  skip_final_snapshot = true

  performance_insights_enabled = true

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}-replica"
    Role = "read-replica"
  })
}

# Outputs
output "endpoint" {
  description = "RDS endpoint"
  value       = aws_db_instance.main.endpoint
}

output "replica_endpoint" {
  description = "Read replica endpoint"
  value       = var.environment == "prod" ? aws_db_instance.replica[0].endpoint : null
}

output "database_name" {
  description = "Database name"
  value       = aws_db_instance.main.db_name
}

output "master_username" {
  description = "Master username"
  value       = aws_db_instance.main.username
  sensitive   = true
}

output "password_secret_arn" {
  description = "ARN of secret containing database password"
  value       = aws_secretsmanager_secret.db_password.arn
}
