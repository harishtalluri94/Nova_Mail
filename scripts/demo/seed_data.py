#!/usr/bin/env python3
"""
Demo Data Seeding Script
Populates Nova Mail with sample tenants, users, and emails for testing and demos
"""

import os
import sys
import uuid
import random
import requests
from datetime import datetime, timedelta
from faker import Faker

fake = Faker()

# Configuration
ADMIN_API_URL = os.getenv("ADMIN_API_URL", "http://localhost:8080/api/v1")
SMTP_HOST = os.getenv("SMTP_HOST", "mail1.nova-mail.test")
SMTP_PORT = int(os.getenv("SMTP_PORT", "25"))

# Demo configuration
NUM_TENANTS = 3
USERS_PER_TENANT = 5
EMAILS_PER_USER = 20


class DemoDataSeeder:
    def __init__(self):
        self.tenants = []
        self.domains = []
        self.users = []
        self.email_subjects = []

    def create_tenant(self, name):
        """Create a demo tenant"""
        print(f"Creating tenant: {name}")

        # Create a dummy plan first
        plan_id = str(uuid.uuid4())

        resp = requests.post(
            f"{ADMIN_API_URL}/tenants",
            json={
                "name": name,
                "plan_id": plan_id,
                "settings": {
                    "max_users": 100,
                    "max_domains": 10,
                    "storage_gb": 1000,
                },
            },
        )

        if resp.status_code != 201:
            print(f"Failed to create tenant: {resp.text}")
            sys.exit(1)

        tenant = resp.json()
        self.tenants.append(tenant)
        print(f"✓ Created tenant: {tenant['id']}")
        return tenant

    def create_domain(self, tenant_id, domain_name):
        """Create a demo domain"""
        print(f"Creating domain: {domain_name}")

        resp = requests.post(
            f"{ADMIN_API_URL}/domains",
            json={"tenant_id": tenant_id, "domain": domain_name},
        )

        if resp.status_code != 201:
            print(f"Failed to create domain: {resp.text}")
            sys.exit(1)

        domain = resp.json()
        self.domains.append(domain)
        print(f"✓ Created domain: {domain['id']}")
        return domain

    def create_user(self, tenant_id, domain_name):
        """Create a demo user"""
        first_name = fake.first_name()
        last_name = fake.last_name()
        email = f"{first_name.lower()}.{last_name.lower()}@{domain_name}"
        password = "DemoPassword123!"

        print(f"Creating user: {email}")

        resp = requests.post(
            f"{ADMIN_API_URL}/users",
            json={
                "tenant_id": tenant_id,
                "email": email,
                "password": password,
                "display_name": f"{first_name} {last_name}",
                "quota_bytes": 10 * 1024 * 1024 * 1024,  # 10 GB
                "quota_messages": 100000,
            },
        )

        if resp.status_code != 201:
            print(f"Failed to create user: {resp.text}")
            sys.exit(1)

        user = resp.json()
        user["password"] = password  # Store for email sending
        self.users.append(user)
        print(f"✓ Created user: {email}")
        return user

    def generate_email_subject(self):
        """Generate realistic email subjects"""
        templates = [
            "Re: {topic}",
            "Fw: {topic}",
            "Meeting: {topic} on {date}",
            "Action Required: {topic}",
            "Update on {topic}",
            "Question about {topic}",
            "{topic} - Please review",
            "Quick question",
            "Follow up from our call",
            "Weekly report - {date}",
            "{topic} proposal",
            "Invitation: {topic}",
            "Reminder: {topic} deadline",
        ]

        topics = [
            "Q4 Results",
            "Product Launch",
            "Team Meeting",
            "Budget Review",
            "Client Presentation",
            "Project Status",
            "Marketing Campaign",
            "Engineering Sprint",
            "Customer Feedback",
            "Sales Pipeline",
            "Roadmap Planning",
            "Performance Review",
        ]

        template = random.choice(templates)
        topic = random.choice(topics)
        date = fake.date_between(start_date="-30d", end_date="+30d").strftime("%B %d")

        return template.format(topic=topic, date=date)

    def generate_email_body(self):
        """Generate realistic email body"""
        templates = [
            """Hi team,

{paragraph1}

{paragraph2}

{paragraph3}

Best regards,
{sender}""",
            """Hello,

Quick update on {topic}:

{paragraph1}

Let me know if you have any questions.

Thanks,
{sender}""",
            """Team,

Just a reminder about {topic}.

{paragraph1}

{paragraph2}

Cheers,
{sender}""",
        ]

        template = random.choice(templates)
        return template.format(
            paragraph1=fake.paragraph(),
            paragraph2=fake.paragraph(),
            paragraph3=fake.paragraph(),
            topic=random.choice(
                [
                    "the upcoming deadline",
                    "our discussion",
                    "the project",
                    "the meeting",
                ]
            ),
            sender=fake.first_name(),
        )

    def send_demo_email(self, from_email, to_email):
        """Send a demo email via SMTP"""
        import smtplib
        from email.mime.text import MIMEText

        subject = self.generate_email_subject()
        body = self.generate_email_body()

        msg = MIMEText(body)
        msg["Subject"] = subject
        msg["From"] = from_email
        msg["To"] = to_email
        msg["Date"] = (
            datetime.now() - timedelta(days=random.randint(0, 30))
        ).strftime("%a, %d %b %Y %H:%M:%S %z")

        try:
            with smtplib.SMTP(SMTP_HOST, SMTP_PORT, timeout=10) as smtp:
                smtp.sendmail(from_email, [to_email], msg.as_string())
            return True
        except Exception as e:
            print(f"Failed to send email: {e}")
            return False

    def seed_all(self):
        """Seed all demo data"""
        print("=" * 60)
        print("Nova Mail Demo Data Seeder")
        print("=" * 60)

        # Create tenants and their data
        for i in range(NUM_TENANTS):
            company_name = f"{fake.company()} (Demo {i + 1})"
            domain_name = f"demo{i + 1}.example.com"

            # Create tenant
            tenant = self.create_tenant(company_name)

            # Create domain
            domain = self.create_domain(tenant["id"], domain_name)

            # Create users
            tenant_users = []
            for j in range(USERS_PER_TENANT):
                user = self.create_user(tenant["id"], domain_name)
                tenant_users.append(user)

            # Send emails between users
            print(f"\nSending demo emails for {domain_name}...")
            emails_sent = 0
            for user in tenant_users:
                for k in range(EMAILS_PER_USER):
                    # Pick random sender and recipient
                    from_user = random.choice(tenant_users)
                    to_user = user

                    if self.send_demo_email(from_user["email"], to_user["email"]):
                        emails_sent += 1
                        if emails_sent % 10 == 0:
                            print(f"Sent {emails_sent} emails...")

            print(f"✓ Sent {emails_sent} demo emails for {domain_name}")

        print("\n" + "=" * 60)
        print("Demo Data Seeding Complete!")
        print("=" * 60)
        print(f"\nCreated:")
        print(f"  - {len(self.tenants)} tenants")
        print(f"  - {len(self.domains)} domains")
        print(f"  - {len(self.users)} users")
        print(f"  - ~{NUM_TENANTS * USERS_PER_TENANT * EMAILS_PER_USER} emails")

        print("\n" + "Sample Users:")
        for user in self.users[:5]:
            print(f"  - {user['email']} (password: {user['password']})")

        print("\nYou can now log in to the webmail with any of these users!")


if __name__ == "__main__":
    try:
        from faker import Faker
    except ImportError:
        print("Error: This script requires the 'faker' package.")
        print("Install it with: pip install faker")
        sys.exit(1)

    seeder = DemoDataSeeder()
    seeder.seed_all()
