'use client';

import { useEffect } from 'react';
import { useMailStore } from '@/store/mail-store';
import { jmapClient, Email } from '@/lib/jmap-client';
import {
  PaperClipIcon,
  StarIcon as StarIconOutline,
} from '@heroicons/react/24/outline';
import { StarIcon as StarIconSolid } from '@heroicons/react/24/solid';
import { formatDistanceToNow } from 'date-fns';

export function EmailList() {
  const {
    selectedMailbox,
    emails,
    selectedEmail,
    loadingEmails,
    setEmails,
    selectEmail,
    setLoadingEmails,
  } = useMailStore();

  useEffect(() => {
    if (selectedMailbox) {
      loadEmails();
    }
  }, [selectedMailbox]);

  async function loadEmails() {
    if (!selectedMailbox) return;

    setLoadingEmails(true);
    try {
      const { emails: fetchedEmails } = await jmapClient.getEmails(
        selectedMailbox.id,
        50,
        0
      );
      setEmails(fetchedEmails);
    } catch (error) {
      console.error('Failed to load emails:', error);
    } finally {
      setLoadingEmails(false);
    }
  }

  async function toggleStar(email: Email, e: React.MouseEvent) {
    e.stopPropagation();

    const isFlagged = email.keywords?.['$flagged'] || false;
    const newKeywords = {
      ...email.keywords,
      $flagged: !isFlagged,
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
      console.error('Failed to toggle star:', error);
    }
  }

  if (!selectedMailbox) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-500">
        Select a mailbox to view emails
      </div>
    );
  }

  if (loadingEmails) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  if (emails.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-gray-500">
        <svg
          className="w-24 h-24 mb-4 text-gray-300"
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
        <p className="text-lg font-medium">No emails</p>
        <p className="text-sm">This mailbox is empty</p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto bg-white">
      {emails.map((email) => {
        const isSelected = selectedEmail?.id === email.id;
        const isUnread = !email.keywords?.['$seen'];
        const isFlagged = email.keywords?.['$flagged'];
        const sender = email.from?.[0];

        return (
          <div
            key={email.id}
            onClick={() => selectEmail(email)}
            className={`
              border-b border-gray-200 px-4 py-3 cursor-pointer transition-colors
              ${isSelected ? 'bg-blue-50 border-l-4 border-l-blue-600' : 'hover:bg-gray-50'}
              ${isUnread ? 'bg-blue-50/30' : ''}
            `}
          >
            <div className="flex items-start gap-3">
              {/* Star */}
              <button
                onClick={(e) => toggleStar(email, e)}
                className="mt-1 flex-shrink-0"
              >
                {isFlagged ? (
                  <StarIconSolid className="w-5 h-5 text-yellow-500" />
                ) : (
                  <StarIconOutline className="w-5 h-5 text-gray-400 hover:text-yellow-500" />
                )}
              </button>

              {/* Email content */}
              <div className="flex-1 min-w-0">
                <div className="flex items-baseline justify-between gap-2 mb-1">
                  <span
                    className={`font-medium truncate ${
                      isUnread ? 'text-gray-900' : 'text-gray-700'
                    }`}
                  >
                    {sender?.name || sender?.email || 'Unknown'}
                  </span>
                  <span className="text-xs text-gray-500 flex-shrink-0">
                    {formatDistanceToNow(new Date(email.receivedAt), {
                      addSuffix: true,
                    })}
                  </span>
                </div>

                <div
                  className={`text-sm truncate mb-1 ${
                    isUnread ? 'font-semibold text-gray-900' : 'text-gray-700'
                  }`}
                >
                  {email.subject || '(no subject)'}
                </div>

                <div className="flex items-center gap-2">
                  <p className="text-sm text-gray-500 truncate flex-1">
                    {email.preview}
                  </p>
                  {email.hasAttachment && (
                    <PaperClipIcon className="w-4 h-4 text-gray-400 flex-shrink-0" />
                  )}
                </div>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
