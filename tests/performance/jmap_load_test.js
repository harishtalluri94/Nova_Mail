/**
 * JMAP API Load Test
 * Tests JMAP API performance under load
 * Target: p99 < 200ms as per SLO
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const jmapQueryDuration = new Trend('jmap_query_duration');
const jmapGetDuration = new Trend('jmap_get_duration');

// Test configuration
export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 200 },   // Ramp to 200 users
    { duration: '5m', target: 200 },   // Stay at 200 users
    { duration: '2m', target: 0 },     // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<150', 'p(99)<200'], // SLO: p99 < 200ms
    errors: ['rate<0.01'],  // Error rate < 1%
    http_req_failed: ['rate<0.01'],
  },
};

const BASE_URL = __ENV.JMAP_URL || 'http://localhost:8081';

// Get JMAP session
function getSession() {
  const response = http.get(`${BASE_URL}/session`);

  const success = check(response, {
    'session status is 200': (r) => r.status === 200,
    'session has capabilities': (r) => {
      const body = JSON.parse(r.body);
      return body.capabilities !== undefined;
    },
  });

  if (!success) {
    errorRate.add(1);
    return null;
  }

  const session = JSON.parse(response.body);
  return {
    accountId: Object.keys(session.accounts)[0],
    apiUrl: session.apiUrl,
  };
}

// Query emails
function queryEmails(session) {
  const payload = JSON.stringify({
    using: [
      'urn:ietf:params:jmap:core',
      'urn:ietf:params:jmap:mail',
    ],
    methodCalls: [
      [
        'Email/query',
        {
          accountId: session.accountId,
          sort: [{ property: 'receivedAt', isAscending: false }],
          limit: 50,
        },
        'q1',
      ],
    ],
  });

  const startTime = Date.now();
  const response = http.post(`${BASE_URL}/api`, payload, {
    headers: { 'Content-Type': 'application/json' },
  });
  const duration = Date.now() - startTime;

  jmapQueryDuration.add(duration);

  const success = check(response, {
    'query status is 200': (r) => r.status === 200,
    'query has results': (r) => {
      const body = JSON.parse(r.body);
      return body.methodResponses && body.methodResponses.length > 0;
    },
  });

  if (!success) {
    errorRate.add(1);
    return [];
  }

  const result = JSON.parse(response.body);
  return result.methodResponses[0][1].ids || [];
}

// Get email details
function getEmails(session, emailIds) {
  const payload = JSON.stringify({
    using: [
      'urn:ietf:params:jmap:core',
      'urn:ietf:params:jmap:mail',
    ],
    methodCalls: [
      [
        'Email/get',
        {
          accountId: session.accountId,
          ids: emailIds.slice(0, 10), // Get first 10 emails
          properties: [
            'id',
            'subject',
            'from',
            'to',
            'receivedAt',
            'preview',
            'hasAttachment',
          ],
        },
        'g1',
      ],
    ],
  });

  const startTime = Date.now();
  const response = http.post(`${BASE_URL}/api`, payload, {
    headers: { 'Content-Type': 'application/json' },
  });
  const duration = Date.now() - startTime;

  jmapGetDuration.add(duration);

  const success = check(response, {
    'get status is 200': (r) => r.status === 200,
    'get has emails': (r) => {
      const body = JSON.parse(r.body);
      return body.methodResponses[0][1].list.length > 0;
    },
  });

  if (!success) {
    errorRate.add(1);
  }
}

// Search emails
function searchEmails(session, query) {
  const payload = JSON.stringify({
    using: [
      'urn:ietf:params:jmap:core',
      'urn:ietf:params:jmap:mail',
    ],
    methodCalls: [
      [
        'Email/query',
        {
          accountId: session.accountId,
          filter: { text: query },
          limit: 20,
        },
        'q1',
      ],
    ],
  });

  const response = http.post(`${BASE_URL}/api`, payload, {
    headers: { 'Content-Type': 'application/json' },
  });

  const success = check(response, {
    'search status is 200': (r) => r.status === 200,
  });

  if (!success) {
    errorRate.add(1);
  }
}

// Main test scenario
export default function () {
  // Get session
  const session = getSession();
  if (!session) {
    sleep(1);
    return;
  }

  // Query recent emails (most common operation)
  const emailIds = queryEmails(session);

  sleep(0.5);

  // Get email details if we have results
  if (emailIds.length > 0) {
    getEmails(session, emailIds);
  }

  sleep(0.5);

  // Occasionally perform searches (30% of requests)
  if (Math.random() < 0.3) {
    const queries = ['important', 'project', 'invoice', 'meeting', 'update'];
    const randomQuery = queries[Math.floor(Math.random() * queries.length)];
    searchEmails(session, randomQuery);
  }

  sleep(1);
}

// Summary at the end
export function handleSummary(data) {
  return {
    'stdout': JSON.stringify(data, null, 2),
    'summary.json': JSON.stringify(data),
  };
}
