import { create } from 'zustand';
import { Email, Mailbox } from '@/lib/jmap-client';

interface MailState {
  // Mailboxes
  mailboxes: Mailbox[];
  selectedMailbox: Mailbox | null;
  setMailboxes: (mailboxes: Mailbox[]) => void;
  selectMailbox: (mailbox: Mailbox) => void;

  // Emails
  emails: Email[];
  selectedEmail: Email | null;
  loadingEmails: boolean;
  setEmails: (emails: Email[]) => void;
  selectEmail: (email: Email | null) => void;
  setLoadingEmails: (loading: boolean) => void;

  // Search
  searchQuery: string;
  searchResults: Email[];
  isSearching: boolean;
  setSearchQuery: (query: string) => void;
  setSearchResults: (results: Email[]) => void;
  setIsSearching: (searching: boolean) => void;

  // Compose
  isComposing: boolean;
  composeDraft: ComposeD raft | null;
  setIsComposing: (composing: boolean) => void;
  setComposeDraft: (draft: ComposeDraft | null) => void;

  // UI State
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
}

export interface ComposeDraft {
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body: string;
  attachments: File[];
}

export const useMailStore = create<MailState>((set) => ({
  // Mailboxes
  mailboxes: [],
  selectedMailbox: null,
  setMailboxes: (mailboxes) => set({ mailboxes }),
  selectMailbox: (mailbox) => set({ selectedMailbox: mailbox, selectedEmail: null }),

  // Emails
  emails: [],
  selectedEmail: null,
  loadingEmails: false,
  setEmails: (emails) => set({ emails }),
  selectEmail: (email) => set({ selectedEmail: email }),
  setLoadingEmails: (loading) => set({ loadingEmails: loading }),

  // Search
  searchQuery: '',
  searchResults: [],
  isSearching: false,
  setSearchQuery: (query) => set({ searchQuery: query }),
  setSearchResults: (results) => set({ searchResults: results }),
  setIsSearching: (searching) => set({ isSearching: searching }),

  // Compose
  isComposing: false,
  composeDraft: null,
  setIsComposing: (composing) => set({ isComposing: composing }),
  setComposeDraft: (draft) => set({ composeDraft: draft }),

  // UI State
  sidebarCollapsed: false,
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
}));
