'use client';

import { useEffect, useState } from 'react';
import { useMailStore } from '@/store/mail-store';
import { jmapClient, Email } from '@/lib/jmap-client';
import {
  ArrowUturnLeftIcon,
  ArrowUturnRightIcon,
  TrashIcon,
  ArchiveBoxIcon,
  EllipsisVerticalIcon,
  PaperClipIcon,
} from '@heroicons/react/24/outline';
import { format } from 'date-fns';

export function EmailViewer() {
  const { selectedEmail, selectEmail, emails, setEmails } = useMailStore();
  const [fullEmail, setFullEmail] = useState<Email | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (selectedEmail) {
      loadFullEmail(selectedEmail.id);
      markAsRead(selectedEmail);
    } else {
      setFullEmail(null);
    }
  }, [selectedEmail?.id]);

  async function loadFullEmail(emailId: string) {
    setLoading(true);
    try {
      const email = await jmapClient.getEmail(emailId);
      setFullEmail(email);
    } catch (error) {
      console.error('Failed to load email:', error);
    } finally {
      setLoading(false);
    }
  }

  async function markAsRead(email: Email) {
    if (email.keywords?.['$seen']) return;

    const newKeywords = {
      ...email.keywords,
      $seen: true,
    };

    try {
      await jmapClient.setKeywords(email.id, newKeywords);
      // Update local state
      setEmails(
        emails.map((e) =>
          e.id === email.id ? { ...e, keywords: newKeywords } : e
        )
      );
    } catch (error) {
      console.error('Failed to mark as read:', error);
    }
  }

  async function handleDelete() {
    if (!selectedEmail) return;

    try {
      await jmapClient.deleteEmail(selectedEmail.id);
      setEmails(emails.filter((e) => e.id !== selectedEmail.id));
      selectEmail(null);
    } catch (error) {
      console.error('Failed to delete email:', error);
    }
  }

  async function handleArchive() {
    if (!selectedEmail) return;

    try {
      // Find archive mailbox
      const mailboxes = await jmapClient.getMailboxes();
      const archiveMailbox = mailboxes.find((m) => m.role === 'archive');

      if (archiveMailbox) {
        await jmapClient.moveToMailbox(selectedEmail.id, archiveMailbox.id);
        setEmails(emails.filter((e) => e.id !== selectedEmail.id));
        selectEmail(null);
      }
    } catch (error) {
      console.error('Failed to archive email:', error);
    }
  }

  if (!selectedEmail) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-500">
        <div className="text-center">
          <svg
            className="w-24 h-24 mb-4 mx-auto text-gray-300"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
            />
          </svg>
          <p className="text-lg font-medium">No email selected</p>
          <p className="text-sm">Select an email to read</p>
        </div>
      </div>
    );
  }

  const sender = fullEmail?.from?.[0] || selectedEmail.from?.[0];
  const htmlBody = fullEmail?.htmlBody?.[0];
  const textBody = fullEmail?.textBody?.[0];
  const bodyValue = htmlBody || textBody;
  const bodyContent = bodyValue
    ? fullEmail?.bodyValues?.[bodyValue.partId]?.value
    : '';

  return (
    <div className="flex-1 flex flex-col bg-white">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-200">
        <button
          onClick={() => selectEmail(null)}
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          title="Back to list"
        >
          <ArrowUturnLeftIcon className="w-5 h-5 text-gray-600" />
        </button>

        <div className="flex-1" />

        <button
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          title="Reply"
        >
          <ArrowUturnLeftIcon className="w-5 h-5 text-gray-600" />
        </button>

        <button
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          title="Forward"
        >
          <ArrowUturnRightIcon className="w-5 h-5 text-gray-600" />
        </button>

        <button
          onClick={handleArchive}
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          title="Archive"
        >
          <ArchiveBoxIcon className="w-5 h-5 text-gray-600" />
        </button>

        <button
          onClick={handleDelete}
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors text-red-600"
          title="Delete"
        >
          <TrashIcon className="w-5 h-5" />
        </button>

        <button className="p-2 hover:bg-gray-100 rounded-lg transition-colors">
          <EllipsisVerticalIcon className="w-5 h-5 text-gray-600" />
        </button>
      </div>

      {/* Email content */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-4xl mx-auto p-6">
          {/* Subject */}
          <h1 className="text-2xl font-bold mb-4">
            {selectedEmail.subject || '(no subject)'}
          </h1>

          {/* Sender info */}
          <div className="flex items-start gap-4 mb-6 pb-6 border-b border-gray-200">
            <div className="w-12 h-12 rounded-full bg-blue-600 flex items-center justify-center text-white font-semibold text-lg">
              {(sender?.name || sender?.email || '?')[0].toUpperCase()}
            </div>

            <div className="flex-1">
              <div className="flex items-baseline justify-between">
                <div>
                  <div className="font-semibold text-gray-900">
                    {sender?.name || sender?.email}
                  </div>
                  {sender?.name && (
                    <div className="text-sm text-gray-500">{sender.email}</div>
                  )}
                </div>
                <div className="text-sm text-gray-500">
                  {fullEmail &&
                    format(
                      new Date(fullEmail.receivedAt),
                      'MMM d, yyyy h:mm a'
                    )}
                </div>
              </div>

              {fullEmail && fullEmail.to && fullEmail.to.length > 0 && (
                <div className="mt-2 text-sm text-gray-600">
                  <span className="font-medium">To:</span>{' '}
                  {fullEmail.to.map((t) => t.email).join(', ')}
                </div>
              )}

              {fullEmail && fullEmail.cc && fullEmail.cc.length > 0 && (
                <div className="mt-1 text-sm text-gray-600">
                  <span className="font-medium">Cc:</span>{' '}
                  {fullEmail.cc.map((c) => c.email).join(', ')}
                </div>
              )}
            </div>
          </div>

          {/* Attachments */}
          {fullEmail?.attachments && fullEmail.attachments.length > 0 && (
            <div className="mb-6">
              <div className="text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
                <PaperClipIcon className="w-4 h-4" />
                {fullEmail.attachments.length} Attachment
                {fullEmail.attachments.length > 1 ? 's' : ''}
              </div>
              <div className="space-y-2">
                {fullEmail.attachments.map((att, idx) => (
                  <a
                    key={idx}
                    href={jmapClient.getDownloadUrl(att.blobId, att.name)}
                    className="flex items-center gap-3 p-3 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors"
                    download
                  >
                    <div className="w-10 h-10 bg-gray-100 rounded flex items-center justify-center">
                      <PaperClipIcon className="w-5 h-5 text-gray-600" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="font-medium text-sm truncate">
                        {att.name || 'Attachment'}
                      </div>
                      <div className="text-xs text-gray-500">
                        {formatBytes(att.size)}
                      </div>
                    </div>
                  </a>
                ))}
              </div>
            </div>
          )}

          {/* Body */}
          {loading ? (
            <div className="flex justify-center py-12">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
            </div>
          ) : (
            <div className="prose max-w-none">
              {htmlBody ? (
                <div dangerouslySetInnerHTML={{ __html: bodyContent || '' }} />
              ) : (
                <pre className="whitespace-pre-wrap font-sans">
                  {bodyContent || selectedEmail.preview}
                </pre>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 Bytes';

  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
}
