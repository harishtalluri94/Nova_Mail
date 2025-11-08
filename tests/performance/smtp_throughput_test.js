/**
 * SMTP Throughput Test
 * Tests email delivery throughput
 * Target: 1000+ emails/minute
 */

import { check } from 'k6';
import smtp from 'k6/x/smtp';
import { Rate, Counter, Trend } from 'k6/metrics';

// Custom metrics
const emailsSent = new Counter('emails_sent');
const emailsFailed = new Counter('emails_failed');
const smtpDuration = new Trend('smtp_duration');

export const options = {
  scenarios: {
    constant_rate: {
      executor: 'constant-arrival-rate',
      rate: 20, // 20 emails per second = 1200/minute
      timeUnit: '1s',
      duration: '5m',
      preAllocatedVUs: 50,
      maxVUs: 100,
    },
  },
  thresholds: {
    emails_sent: ['count>5000'], // At least 5000 emails in 5 minutes
    emails_failed: ['count<50'], // Less than 50 failures
    smtp_duration: ['p(95)<1000'], // p95 delivery < 1s
  },
};

const SMTP_HOST = __ENV.SMTP_HOST || 'mail1.nova-mail.test';
const SMTP_PORT = parseInt(__ENV.SMTP_PORT || '25');
const RECIPIENT = __ENV.TEST_RECIPIENT || 'test@nova-mail.test';

export default function () {
  const subject = `Load Test Email ${Date.now()}-${__VU}-${__ITER}`;
  const body = `This is a load test email.

Virtual User: ${__VU}
Iteration: ${__ITER}
Timestamp: ${new Date().toISOString()}

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod
tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam,
quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo
consequat.`;

  const startTime = Date.now();

  try {
    const response = smtp.send({
      host: SMTP_HOST,
      port: SMTP_PORT,
      from: 'loadtest@example.com',
      to: [RECIPIENT],
      subject: subject,
      body: body,
    });

    const duration = Date.now() - startTime;
    smtpDuration.add(duration);

    const success = check(response, {
      'SMTP accepted': (r) => r.status === 'OK',
    });

    if (success) {
      emailsSent.add(1);
    } else {
      emailsFailed.add(1);
    }
  } catch (error) {
    console.error(`SMTP error: ${error}`);
    emailsFailed.add(1);
  }
}

export function handleSummary(data) {
  const sent = data.metrics.emails_sent.values.count;
  const failed = data.metrics.emails_failed.values.count;
  const duration = data.state.testRunDurationMs / 1000 / 60; // minutes

  console.log(`\n=== SMTP Throughput Test Results ===`);
  console.log(`Emails sent: ${sent}`);
  console.log(`Emails failed: ${failed}`);
  console.log(`Success rate: ${((sent / (sent + failed)) * 100).toFixed(2)}%`);
  console.log(`Throughput: ${(sent / duration).toFixed(0)} emails/minute`);
  console.log(`p95 duration: ${data.metrics.smtp_duration.values['p(95)'].toFixed(0)}ms`);
  console.log(`p99 duration: ${data.metrics.smtp_duration.values['p(99)'].toFixed(0)}ms`);

  return {
    'stdout': JSON.stringify(data, null, 2),
    'smtp_summary.json': JSON.stringify(data),
  };
}
