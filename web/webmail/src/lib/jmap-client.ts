/**
 * JMAP Client SDK for Nova Mail
 * Implements JMAP Core (RFC 8620) and JMAP Mail (RFC 8621)
 */

export interface JmapSession {
  capabilities: Record<string, any>;
  accounts: Record<string, JmapAccount>;
  primaryAccounts: Record<string, string>;
  username: string;
  apiUrl: string;
  downloadUrl: string;
  uploadUrl: string;
  eventSourceUrl: string;
  state: string;
}

export interface JmapAccount {
  name: string;
  isPersonal: boolean;
  isReadOnly: boolean;
  accountCapabilities: Record<string, any>;
}

export interface JmapRequest {
  using: string[];
  methodCalls: JmapMethodCall[];
}

export type JmapMethodCall = [string, any, string];

export interface JmapResponse {
  methodResponses: JmapMethodResponse[];
  sessionState: string;
}

export type JmapMethodResponse = [string, any, string];

export interface Email {
  id: string;
  blobId: string;
  threadId: string;
  mailboxIds: Record<string, boolean>;
  keywords: Record<string, boolean>;
  size: number;
  receivedAt: string;
  messageId: string[];
  inReplyTo: string[];
  references: string[];
  sender: EmailAddress[];
  from: EmailAddress[];
  to: EmailAddress[];
  cc: EmailAddress[];
  bcc: EmailAddress[];
  replyTo: EmailAddress[];
  subject: string;
  sentAt: string;
  hasAttachment: boolean;
  preview: string;
  bodyStructure: BodyPart;
  bodyValues?: Record<string, BodyValue>;
  textBody?: BodyPart[];
  htmlBody?: BodyPart[];
  attachments?: BodyPart[];
}

export interface EmailAddress {
  name?: string;
  email: string;
}

export interface BodyPart {
  partId: string;
  blobId: string;
  size: number;
  name?: string;
  type: string;
  charset?: string;
  disposition?: string;
  cid?: string;
  language?: string[];
  location?: string;
  subParts?: BodyPart[];
}

export interface BodyValue {
  value: string;
  isEncodingProblem: boolean;
  isTruncated: boolean;
}

export interface Mailbox {
  id: string;
  name: string;
  parentId: string | null;
  role: string | null;
  sortOrder: number;
  totalEmails: number;
  unreadEmails: number;
  totalThreads: number;
  unreadThreads: number;
  myRights: MailboxRights;
  isSubscribed: boolean;
}

export interface MailboxRights {
  mayReadItems: boolean;
  mayAddItems: boolean;
  mayRemoveItems: boolean;
  maySetSeen: boolean;
  maySetKeywords: boolean;
  mayCreateChild: boolean;
  mayRename: boolean;
  mayDelete: boolean;
  maySubmit: boolean;
}

export interface Thread {
  id: string;
  emailIds: string[];
}

export class JmapClient {
  private session: JmapSession | null = null;
  private baseUrl: string;
  private accountId: string | null = null;

  constructor(baseUrl: string = '/api/jmap') {
    this.baseUrl = baseUrl;
  }

  async getSession(): Promise<JmapSession> {
    if (this.session) {
      return this.session;
    }

    const response = await fetch(`${this.baseUrl}/session`, {
      credentials: 'include',
    });

    if (!response.ok) {
      throw new Error(`Failed to get session: ${response.statusText}`);
    }

    this.session = await response.json();
    this.accountId = Object.keys(this.session!.accounts)[0];

    return this.session!;
  }

  async call(methodCalls: JmapMethodCall[]): Promise<JmapResponse> {
    if (!this.session) {
      await this.getSession();
    }

    const request: JmapRequest = {
      using: [
        'urn:ietf:params:jmap:core',
        'urn:ietf:params:jmap:mail',
      ],
      methodCalls,
    };

    const response = await fetch(`${this.baseUrl}/api`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      credentials: 'include',
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error(`JMAP request failed: ${response.statusText}`);
    }

    return await response.json();
  }

  async getMailboxes(): Promise<Mailbox[]> {
    const response = await this.call([
      [
        'Mailbox/query',
        {
          accountId: this.accountId,
          sort: [{ property: 'sortOrder', isAscending: true }],
        },
        'q1',
      ],
      [
        'Mailbox/get',
        {
          accountId: this.accountId,
          '#ids': {
            resultOf: 'q1',
            name: 'Mailbox/query',
            path: '/ids',
          },
        },
        'g1',
      ],
    ]);

    const getResponse = response.methodResponses.find(
      ([name]) => name === 'Mailbox/get'
    );

    if (!getResponse) {
      throw new Error('Mailbox/get response not found');
    }

    return getResponse[1].list;
  }

  async getEmails(
    mailboxId: string,
    limit: number = 50,
    position: number = 0
  ): Promise<{ emails: Email[]; total: number }> {
    const response = await this.call([
      [
        'Email/query',
        {
          accountId: this.accountId,
          filter: { inMailbox: mailboxId },
          sort: [{ property: 'receivedAt', isAscending: false }],
          limit,
          position,
        },
        'q1',
      ],
      [
        'Email/get',
        {
          accountId: this.accountId,
          '#ids': {
            resultOf: 'q1',
            name: 'Email/query',
            path: '/ids',
          },
          properties: [
            'id',
            'threadId',
            'mailboxIds',
            'keywords',
            'size',
            'receivedAt',
            'from',
            'to',
            'cc',
            'subject',
            'preview',
            'hasAttachment',
          ],
        },
        'g1',
      ],
    ]);

    const queryResponse = response.methodResponses.find(
      ([name]) => name === 'Email/query'
    );
    const getResponse = response.methodResponses.find(
      ([name]) => name === 'Email/get'
    );

    if (!queryResponse || !getResponse) {
      throw new Error('Email responses not found');
    }

    return {
      emails: getResponse[1].list,
      total: queryResponse[1].total,
    };
  }

  async getEmail(emailId: string): Promise<Email> {
    const response = await this.call([
      [
        'Email/get',
        {
          accountId: this.accountId,
          ids: [emailId],
          properties: [
            'id',
            'blobId',
            'threadId',
            'mailboxIds',
            'keywords',
            'size',
            'receivedAt',
            'messageId',
            'inReplyTo',
            'references',
            'from',
            'to',
            'cc',
            'bcc',
            'subject',
            'sentAt',
            'hasAttachment',
            'preview',
            'bodyStructure',
            'bodyValues',
            'textBody',
            'htmlBody',
            'attachments',
          ],
          bodyProperties: ['partId', 'blobId', 'size', 'type', 'charset', 'name', 'disposition'],
          fetchAllBodyValues: true,
        },
        'g1',
      ],
    ]);

    const getResponse = response.methodResponses.find(
      ([name]) => name === 'Email/get'
    );

    if (!getResponse || getResponse[1].list.length === 0) {
      throw new Error('Email not found');
    }

    return getResponse[1].list[0];
  }

  async searchEmails(
    query: string,
    limit: number = 50
  ): Promise<{ emails: Email[]; total: number }> {
    const response = await this.call([
      [
        'Email/query',
        {
          accountId: this.accountId,
          filter: {
            text: query,
          },
          sort: [{ property: 'receivedAt', isAscending: false }],
          limit,
        },
        'q1',
      ],
      [
        'Email/get',
        {
          accountId: this.accountId,
          '#ids': {
            resultOf: 'q1',
            name: 'Email/query',
            path: '/ids',
          },
          properties: [
            'id',
            'threadId',
            'mailboxIds',
            'from',
            'to',
            'subject',
            'preview',
            'receivedAt',
            'hasAttachment',
          ],
        },
        'g1',
      ],
    ]);

    const queryResponse = response.methodResponses.find(
      ([name]) => name === 'Email/query'
    );
    const getResponse = response.methodResponses.find(
      ([name]) => name === 'Email/get'
    );

    if (!queryResponse || !getResponse) {
      throw new Error('Search responses not found');
    }

    return {
      emails: getResponse[1].list,
      total: queryResponse[1].total,
    };
  }

  async setKeywords(
    emailId: string,
    keywords: Record<string, boolean>
  ): Promise<void> {
    await this.call([
      [
        'Email/set',
        {
          accountId: this.accountId,
          update: {
            [emailId]: {
              keywords,
            },
          },
        },
        's1',
      ],
    ]);
  }

  async moveToMailbox(emailId: string, mailboxId: string): Promise<void> {
    await this.call([
      [
        'Email/set',
        {
          accountId: this.accountId,
          update: {
            [emailId]: {
              mailboxIds: { [mailboxId]: true },
            },
          },
        },
        's1',
      ],
    ]);
  }

  async deleteEmail(emailId: string): Promise<void> {
    await this.call([
      [
        'Email/set',
        {
          accountId: this.accountId,
          destroy: [emailId],
        },
        's1',
      ],
    ]);
  }

  async sendEmail(email: {
    from: EmailAddress[];
    to: EmailAddress[];
    cc?: EmailAddress[];
    bcc?: EmailAddress[];
    subject: string;
    textBody?: string;
    htmlBody?: string;
    attachments?: { blobId: string; name: string; type: string }[];
  }): Promise<string> {
    // Create email draft
    const bodyStructure: BodyPart = {
      partId: '1',
      type: email.htmlBody ? 'text/html' : 'text/plain',
      charset: 'utf-8',
    } as BodyPart;

    const response = await this.call([
      [
        'Email/set',
        {
          accountId: this.accountId,
          create: {
            draft: {
              from: email.from,
              to: email.to,
              cc: email.cc || [],
              bcc: email.bcc || [],
              subject: email.subject,
              bodyStructure,
              bodyValues: {
                '1': {
                  value: email.htmlBody || email.textBody || '',
                },
              },
              keywords: { $draft: true },
            },
          },
        },
        's1',
      ],
      [
        'EmailSubmission/set',
        {
          accountId: this.accountId,
          create: {
            sub1: {
              emailId: '#draft',
              identityId: this.accountId,
            },
          },
        },
        's2',
      ],
    ]);

    const submissionResponse = response.methodResponses.find(
      ([name]) => name === 'EmailSubmission/set'
    );

    if (!submissionResponse || !submissionResponse[1].created) {
      throw new Error('Failed to send email');
    }

    return submissionResponse[1].created.sub1.id;
  }

  async uploadBlob(file: File): Promise<string> {
    if (!this.session) {
      await this.getSession();
    }

    const response = await fetch(
      `${this.session!.uploadUrl.replace('{accountId}', this.accountId!)}`,
      {
        method: 'POST',
        headers: {
          'Content-Type': file.type,
        },
        credentials: 'include',
        body: file,
      }
    );

    if (!response.ok) {
      throw new Error(`Failed to upload blob: ${response.statusText}`);
    }

    const result = await response.json();
    return result.blobId;
  }

  getDownloadUrl(blobId: string, name?: string): string {
    if (!this.session || !this.accountId) {
      throw new Error('Session not initialized');
    }

    let url = this.session.downloadUrl
      .replace('{accountId}', this.accountId)
      .replace('{blobId}', blobId)
      .replace('{name}', name || 'download');

    return url;
  }
}

export const jmapClient = new JmapClient();
