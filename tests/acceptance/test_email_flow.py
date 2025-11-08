#!/usr/bin/env python3
"""
End-to-end acceptance tests for Nova Mail
Tests the complete email flow: SMTP → LMTP → Indexing → JMAP retrieval
"""

import os
import time
import smtplib
import requests
import uuid
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
from email.mime.base import MIMEBase
from email import encoders


class NovaMailAcceptanceTest:
    """Complete email flow acceptance test suite"""

    def __init__(self):
        self.smtp_host = os.getenv("SMTP_HOST", "mail1.nova-mail.test")
        self.smtp_port = int(os.getenv("SMTP_PORT", "25"))
        self.jmap_url = os.getenv("JMAP_URL", "http://localhost:8081")
        self.admin_url = os.getenv("ADMIN_URL", "http://localhost:8080")

        self.test_tenant_id = None
        self.test_domain = "test-" + str(uuid.uuid4())[:8] + ".example.com"
        self.test_user_email = f"test@{self.test_domain}"
        self.test_password = "TestPassword123!"
        self.session_token = None

    def setup(self):
        """Create test tenant, domain, and user"""
        print("Setting up test environment...")

        # Create tenant
        resp = requests.post(
            f"{self.admin_url}/api/v1/tenants",
            json={
                "name": f"Test Tenant {uuid.uuid4()}",
                "plan_id": str(uuid.uuid4()),
            },
        )
        assert resp.status_code == 201, f"Failed to create tenant: {resp.text}"
        self.test_tenant_id = resp.json()["id"]
        print(f"Created tenant: {self.test_tenant_id}")

        # Create domain
        resp = requests.post(
            f"{self.admin_url}/api/v1/domains",
            json={"tenant_id": self.test_tenant_id, "domain": self.test_domain},
        )
        assert resp.status_code == 201, f"Failed to create domain: {resp.text}"
        print(f"Created domain: {self.test_domain}")

        # Create user
        resp = requests.post(
            f"{self.admin_url}/api/v1/users",
            json={
                "tenant_id": self.test_tenant_id,
                "email": self.test_user_email,
                "password": self.test_password,
                "display_name": "Test User",
                "quota_bytes": 10 * 1024 * 1024 * 1024,  # 10 GB
                "quota_messages": 100000,
            },
        )
        assert resp.status_code == 201, f"Failed to create user: {resp.text}"
        print(f"Created user: {self.test_user_email}")

    def test_01_send_simple_email(self):
        """Test sending a simple text email via SMTP"""
        print("\nTest 1: Send simple text email...")

        msg = MIMEText("This is a test email body.")
        msg["Subject"] = "Test Email " + str(uuid.uuid4())[:8]
        msg["From"] = "sender@example.com"
        msg["To"] = self.test_user_email

        with smtplib.SMTP(self.smtp_host, self.smtp_port) as smtp:
            smtp.sendmail("sender@example.com", [self.test_user_email], msg.as_string())

        print("Email sent successfully")
        return msg["Subject"]

    def test_02_wait_for_indexing(self, subject, timeout=30):
        """Wait for email to be indexed and searchable"""
        print(f"\nTest 2: Waiting for email to be indexed (subject: {subject})...")

        # Get JMAP session
        resp = requests.get(f"{self.jmap_url}/session")
        assert resp.status_code == 200, f"Failed to get session: {resp.text}"
        session = resp.json()
        account_id = list(session["accounts"].keys())[0]

        # Poll for the email
        start_time = time.time()
        while time.time() - start_time < timeout:
            # Query for emails
            jmap_req = {
                "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
                "methodCalls": [
                    [
                        "Email/query",
                        {
                            "accountId": account_id,
                            "filter": {"subject": subject},
                        },
                        "q1",
                    ]
                ],
            }

            resp = requests.post(f"{self.jmap_url}/api", json=jmap_req)
            assert resp.status_code == 200, f"JMAP request failed: {resp.text}"

            result = resp.json()
            email_ids = result["methodResponses"][0][1]["ids"]

            if len(email_ids) > 0:
                print(f"Email found and indexed in {time.time() - start_time:.2f}s")
                return email_ids[0]

            time.sleep(1)

        raise AssertionError(f"Email not indexed within {timeout}s")

    def test_03_retrieve_email_via_jmap(self, email_id):
        """Retrieve full email via JMAP"""
        print(f"\nTest 3: Retrieving email via JMAP (id: {email_id})...")

        resp = requests.get(f"{self.jmap_url}/session")
        session = resp.json()
        account_id = list(session["accounts"].keys())[0]

        jmap_req = {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [
                [
                    "Email/get",
                    {
                        "accountId": account_id,
                        "ids": [email_id],
                        "properties": [
                            "id",
                            "subject",
                            "from",
                            "to",
                            "receivedAt",
                            "preview",
                        ],
                    },
                    "g1",
                ]
            ],
        }

        resp = requests.post(f"{self.jmap_url}/api", json=jmap_req)
        assert resp.status_code == 200, f"JMAP request failed: {resp.text}"

        result = resp.json()
        emails = result["methodResponses"][0][1]["list"]

        assert len(emails) == 1, "Email not found"
        email = emails[0]

        print(f"Retrieved email:")
        print(f"  Subject: {email['subject']}")
        print(f"  From: {email['from']}")
        print(f"  To: {email['to']}")
        print(f"  Preview: {email['preview']}")

        return email

    def test_04_send_email_with_attachment(self):
        """Test sending email with attachment"""
        print("\nTest 4: Send email with attachment...")

        msg = MIMEMultipart()
        msg["Subject"] = "Test Attachment " + str(uuid.uuid4())[:8]
        msg["From"] = "sender@example.com"
        msg["To"] = self.test_user_email

        # Add body
        body = MIMEText("This email contains an attachment.")
        msg.attach(body)

        # Add attachment
        attachment_content = b"This is a test file content.\nLine 2\nLine 3"
        part = MIMEBase("application", "octet-stream")
        part.set_payload(attachment_content)
        encoders.encode_base64(part)
        part.add_header(
            "Content-Disposition", 'attachment; filename="test-file.txt"'
        )
        msg.attach(part)

        with smtplib.SMTP(self.smtp_host, self.smtp_port) as smtp:
            smtp.sendmail("sender@example.com", [self.test_user_email], msg.as_string())

        print("Email with attachment sent successfully")
        return msg["Subject"]

    def test_05_search_emails(self, query):
        """Test email search functionality"""
        print(f"\nTest 5: Searching for emails with query: {query}...")

        resp = requests.get(f"{self.jmap_url}/session")
        session = resp.json()
        account_id = list(session["accounts"].keys())[0]

        jmap_req = {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [
                [
                    "Email/query",
                    {
                        "accountId": account_id,
                        "filter": {"text": query},
                    },
                    "q1",
                ]
            ],
        }

        resp = requests.post(f"{self.jmap_url}/api", json=jmap_req)
        assert resp.status_code == 200, f"JMAP search failed: {resp.text}"

        result = resp.json()
        email_ids = result["methodResponses"][0][1]["ids"]

        print(f"Search returned {len(email_ids)} results")
        return email_ids

    def test_06_mark_as_read(self, email_id):
        """Test marking email as read"""
        print(f"\nTest 6: Marking email as read (id: {email_id})...")

        resp = requests.get(f"{self.jmap_url}/session")
        session = resp.json()
        account_id = list(session["accounts"].keys())[0]

        jmap_req = {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [
                [
                    "Email/set",
                    {
                        "accountId": account_id,
                        "update": {email_id: {"keywords": {"$seen": True}}},
                    },
                    "s1",
                ]
            ],
        }

        resp = requests.post(f"{self.jmap_url}/api", json=jmap_req)
        assert resp.status_code == 200, f"JMAP set failed: {resp.text}"

        result = resp.json()
        updated = result["methodResponses"][0][1].get("updated", {})

        assert email_id in updated, "Email not marked as read"
        print("Email marked as read successfully")

    def test_07_delete_email(self, email_id):
        """Test deleting an email"""
        print(f"\nTest 7: Deleting email (id: {email_id})...")

        resp = requests.get(f"{self.jmap_url}/session")
        session = resp.json()
        account_id = list(session["accounts"].keys())[0]

        jmap_req = {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [
                [
                    "Email/set",
                    {"accountId": account_id, "destroy": [email_id]},
                    "s1",
                ]
            ],
        }

        resp = requests.post(f"{self.jmap_url}/api", json=jmap_req)
        assert resp.status_code == 200, f"JMAP delete failed: {resp.text}"

        result = resp.json()
        destroyed = result["methodResponses"][0][1].get("destroyed", [])

        assert email_id in destroyed, "Email not deleted"
        print("Email deleted successfully")

    def run_all_tests(self):
        """Run all acceptance tests in sequence"""
        print("=" * 60)
        print("Nova Mail Acceptance Test Suite")
        print("=" * 60)

        try:
            self.setup()

            # Test 1: Send simple email
            subject1 = self.test_01_send_simple_email()

            # Test 2: Wait for indexing
            email_id1 = self.test_02_wait_for_indexing(subject1)

            # Test 3: Retrieve email
            email = self.test_03_retrieve_email_via_jmap(email_id1)
            assert email["subject"] == subject1

            # Test 4: Send email with attachment
            subject2 = self.test_04_send_email_with_attachment()
            email_id2 = self.test_02_wait_for_indexing(subject2)

            # Test 5: Search emails
            search_results = self.test_05_search_emails("test")
            assert len(search_results) >= 2, "Search should return at least 2 emails"

            # Test 6: Mark as read
            self.test_06_mark_as_read(email_id1)

            # Test 7: Delete email
            self.test_07_delete_email(email_id2)

            print("\n" + "=" * 60)
            print("ALL TESTS PASSED ✓")
            print("=" * 60)

        except Exception as e:
            print("\n" + "=" * 60)
            print(f"TEST FAILED ✗: {e}")
            print("=" * 60)
            raise


if __name__ == "__main__":
    test = NovaMailAcceptanceTest()
    test.run_all_tests()
