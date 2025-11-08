'use client';

import { useEffect } from 'react';
import { useMailStore } from '@/store/mail-store';
import { jmapClient } from '@/lib/jmap-client';
import {
  InboxIcon,
  PaperAirplaneIcon,
  DocumentIcon,
  TrashIcon,
  ArchiveBoxIcon,
  FolderIcon,
  PencilSquareIcon,
} from '@heroicons/react/24/outline';

const ICON_MAP: Record<string, any> = {
  inbox: InboxIcon,
  sent: PaperAirplaneIcon,
  drafts: DocumentIcon,
  trash: TrashIcon,
  archive: ArchiveBoxIcon,
};

export function Sidebar() {
  const {
    mailboxes,
    selectedMailbox,
    setMailboxes,
    selectMailbox,
    sidebarCollapsed,
    setIsComposing,
  } = useMailStore();

  useEffect(() => {
    loadMailboxes();
  }, []);

  async function loadMailboxes() {
    try {
      const boxes = await jmapClient.getMailboxes();
      setMailboxes(boxes);

      // Auto-select inbox
      const inbox = boxes.find((b) => b.role === 'inbox');
      if (inbox) {
        selectMailbox(inbox);
      }
    } catch (error) {
      console.error('Failed to load mailboxes:', error);
    }
  }

  if (sidebarCollapsed) {
    return null;
  }

  return (
    <div className="w-64 bg-gray-50 border-r border-gray-200 flex flex-col">
      {/* Compose button */}
      <div className="p-4">
        <button
          onClick={() => setIsComposing(true)}
          className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          <PencilSquareIcon className="w-5 h-5" />
          <span className="font-medium">Compose</span>
        </button>
      </div>

      {/* Mailbox list */}
      <nav className="flex-1 px-2 space-y-1 overflow-y-auto">
        {mailboxes.map((mailbox) => {
          const Icon = ICON_MAP[mailbox.role || ''] || FolderIcon;
          const isSelected = selectedMailbox?.id === mailbox.id;

          return (
            <button
              key={mailbox.id}
              onClick={() => selectMailbox(mailbox)}
              className={`
                w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
                ${
                  isSelected
                    ? 'bg-blue-50 text-blue-700'
                    : 'text-gray-700 hover:bg-gray-100'
                }
              `}
            >
              <Icon className="w-5 h-5 flex-shrink-0" />
              <span className="flex-1 text-left font-medium truncate">
                {mailbox.name}
              </span>
              {mailbox.unreadEmails > 0 && (
                <span
                  className={`
                    px-2 py-0.5 text-xs font-semibold rounded-full
                    ${
                      isSelected
                        ? 'bg-blue-700 text-white'
                        : 'bg-gray-200 text-gray-700'
                    }
                  `}
                >
                  {mailbox.unreadEmails}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Storage info */}
      <div className="p-4 border-t border-gray-200">
        <div className="text-xs text-gray-500">
          <div className="flex justify-between mb-1">
            <span>Storage</span>
            <span>2.5 GB / 15 GB</span>
          </div>
          <div className="w-full bg-gray-200 rounded-full h-1.5">
            <div
              className="bg-blue-600 h-1.5 rounded-full"
              style={{ width: '16.7%' }}
            ></div>
          </div>
        </div>
      </div>
    </div>
  );
}
