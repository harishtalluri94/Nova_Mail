#!/bin/bash
set -euxo pipefail

# User data script for Nova Mail edge-mta instances
# This script runs on first boot to set up Stalwart, Rspamd, and ClamAV

export DEBIAN_FRONTEND=noninteractive

# Update system
apt-get update
apt-get upgrade -y

# Install prerequisites
apt-get install -y \
    curl \
    wget \
    gnupg \
    ca-certificates \
    software-properties-common \
    apt-transport-https \
    systemd \
    unattended-upgrades

# Install Stalwart Mail Server
# See: https://stalw.art/docs/install/linux
curl -fsSL https://stalw.art/install.sh | sh

# Install Rspamd
apt-get install -y rspamd redis-server

# Install ClamAV
apt-get install -y clamav clamav-daemon
systemctl stop clamav-freshclam
freshclam
systemctl start clamav-freshclam
systemctl enable clamav-freshclam clamav-daemon

# Configure automatic security updates
dpkg-reconfigure -plow unattended-upgrades

# Set hostname
INSTANCE_ID=$(ec2-metadata --instance-id | cut -d " " -f 2)
hostnamectl set-hostname "edge-mta-$INSTANCE_ID"

# Create directories
mkdir -p /etc/stalwart/dkim
mkdir -p /var/log/stalwart
mkdir -p /etc/rspamd/local.d

# Set permissions
chown -R stalwart:stalwart /etc/stalwart /var/log/stalwart
chown -R _rspamd:_rspamd /etc/rspamd

# Enable services
systemctl enable stalwart-mail
systemctl enable rspamd
systemctl enable redis-server

# Configure firewall (if ufw is used)
if command -v ufw &> /dev/null; then
    ufw allow 22/tcp
    ufw allow 25/tcp
    ufw allow 465/tcp
    ufw allow 587/tcp
    ufw --force enable
fi

# Install CloudWatch agent (optional, for AWS)
wget https://s3.amazonaws.com/amazoncloudwatch-agent/ubuntu/amd64/latest/amazon-cloudwatch-agent.deb
dpkg -i -E ./amazon-cloudwatch-agent.deb
rm amazon-cloudwatch-agent.deb

echo "Edge MTA setup complete!"
echo "Next steps:"
echo "1. Copy Stalwart configuration to /etc/stalwart/"
echo "2. Generate and install DKIM keys"
echo "3. Configure Rspamd"
echo "4. Start services: systemctl start stalwart-mail rspamd"
