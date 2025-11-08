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
  description = "List of public subnet IDs"
  type        = list(string)
}

variable "instance_type" {
  description = "EC2 instance type"
  type        = string
  default     = "t3.small"
}

variable "instance_count" {
  description = "Number of edge-mta instances"
  type        = number
  default     = 2
}

variable "key_name" {
  description = "SSH key pair name"
  type        = string
}

locals {
  common_tags = {
    Project     = "nova-mail"
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

# AMI for Ubuntu 22.04
data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"] # Canonical

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# Security Group
resource "aws_security_group" "edge_mta" {
  name        = "nova-mail-${var.environment}-edge-mta"
  description = "Security group for edge MTA instances"
  vpc_id      = var.vpc_id

  # SMTP
  ingress {
    from_port   = 25
    to_port     = 25
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "SMTP"
  }

  # Submission
  ingress {
    from_port   = 587
    to_port     = 587
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Submission"
  }

  # Submissions (SSL)
  ingress {
    from_port   = 465
    to_port     = 465
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Submissions SSL"
  }

  # SSH
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"] # In production, restrict this
    description = "SSH"
  }

  # Rspamd
  ingress {
    from_port   = 11332
    to_port     = 11333
    protocol    = "tcp"
    cidr_blocks = ["10.0.0.0/8"]
    description = "Rspamd from VPC"
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}-edge-mta"
  })
}

# IAM Role for SSM access
resource "aws_iam_role" "edge_mta" {
  name = "nova-mail-${var.environment}-edge-mta"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "ec2.amazonaws.com"
      }
    }]
  })

  tags = local.common_tags
}

resource "aws_iam_role_policy_attachment" "ssm_managed" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
  role       = aws_iam_role.edge_mta.name
}

resource "aws_iam_instance_profile" "edge_mta" {
  name = "nova-mail-${var.environment}-edge-mta"
  role = aws_iam_role.edge_mta.name

  tags = local.common_tags
}

# Elastic IPs
resource "aws_eip" "edge_mta" {
  count  = var.instance_count
  domain = "vpc"

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}-edge-mta-${count.index + 1}"
  })
}

# User data script
data "template_file" "user_data" {
  template = file("${path.module}/user-data.sh")

  vars = {
    environment = var.environment
  }
}

# EC2 Instances
resource "aws_instance" "edge_mta" {
  count = var.instance_count

  ami           = data.aws_ami.ubuntu.id
  instance_type = var.instance_type
  subnet_id     = var.subnet_ids[count.index % length(var.subnet_ids)]

  vpc_security_group_ids = [aws_security_group.edge_mta.id]
  iam_instance_profile   = aws_iam_instance_profile.edge_mta.name

  key_name  = var.key_name
  user_data = data.template_file.user_data.rendered

  root_block_device {
    volume_size = 50
    volume_type = "gp3"
    encrypted   = true
  }

  tags = merge(local.common_tags, {
    Name = "nova-mail-${var.environment}-edge-mta-${count.index + 1}"
    Role = "edge-mta"
  })
}

# Associate EIPs
resource "aws_eip_association" "edge_mta" {
  count = var.instance_count

  instance_id   = aws_instance.edge_mta[count.index].id
  allocation_id = aws_eip.edge_mta[count.index].id
}

# Outputs
output "instance_ids" {
  description = "EC2 instance IDs"
  value       = aws_instance.edge_mta[*].id
}

output "public_ips" {
  description = "Public IP addresses"
  value       = aws_eip.edge_mta[*].public_ip
}

output "private_ips" {
  description = "Private IP addresses"
  value       = aws_instance.edge_mta[*].private_ip
}

output "dns_instructions" {
  description = "DNS setup instructions"
  value = <<-EOT
    Configure the following DNS records:

    MX Records:
    ${join("\n    ", [for i in range(var.instance_count) : "${var.environment == "prod" ? "" : "${var.environment}."}example.com. MX ${(i + 1) * 10} mail${i + 1}.${var.environment == "prod" ? "" : "${var.environment}."}example.com."])}

    A Records:
    ${join("\n    ", [for i in range(var.instance_count) : "mail${i + 1}.${var.environment == "prod" ? "" : "${var.environment}."}example.com. A ${aws_eip.edge_mta[i].public_ip}"])}

    PTR Records (set via your hosting provider):
    ${join("\n    ", [for i in range(var.instance_count) : "${aws_eip.edge_mta[i].public_ip} PTR mail${i + 1}.${var.environment == "prod" ? "" : "${var.environment}."}example.com."])}
  EOT
}
