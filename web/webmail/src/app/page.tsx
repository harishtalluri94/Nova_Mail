'use client';

import { Sidebar } from '@/components/Sidebar';
import { EmailList } from '@/components/EmailList';
import { EmailViewer } from '@/components/EmailViewer';
import { Composer } from '@/components/Composer';
import { SearchBar } from '@/components/SearchBar';
import { useMailStore } from '@/store/mail-store';
import {
  Bars3Icon,
  Cog6ToothIcon,
  UserCircleIcon,
} from '@heroicons/react/24/outline';

export default function MailPage() {
  const { toggleSidebar, selectedEmail } = useMailStore();

  return (
    <div className="h-screen flex flex-col bg-gray-100">
      {/* Top bar */}
      <header className="h-16 bg-white border-b border-gray-200 flex items-center px-4 gap-4">
        <button
          onClick={toggleSidebar}
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
        >
          <Bars3Icon className="w-6 h-6 text-gray-600" />
        </button>

        <div className="flex items-center gap-2">
          <div className="w-8 h-8 bg-blue-600 rounded flex items-center justify-center">
            <span className="text-white font-bold text-sm">N</span>
          </div>
          <h1 className="text-xl font-bold text-gray-900">Nova Mail</h1>
        </div>

        <SearchBar />

        <div className="flex items-center gap-2">
          <button className="p-2 hover:bg-gray-100 rounded-lg transition-colors">
            <Cog6ToothIcon className="w-6 h-6 text-gray-600" />
          </button>
          <button className="p-2 hover:bg-gray-100 rounded-lg transition-colors">
            <UserCircleIcon className="w-6 h-6 text-gray-600" />
          </button>
        </div>
      </header>

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">
        <Sidebar />

        <div className="flex-1 flex overflow-hidden">
          {/* Email list - 1/3 width when email is selected, full width otherwise */}
          <div
            className={`${
              selectedEmail ? 'w-1/3' : 'w-full'
            } border-r border-gray-200 flex flex-col transition-all duration-200`}
          >
            <EmailList />
          </div>

          {/* Email viewer - 2/3 width when email is selected, hidden otherwise */}
          {selectedEmail && (
            <div className="flex-1 flex flex-col transition-all duration-200">
              <EmailViewer />
            </div>
          )}
        </div>
      </div>

      {/* Composer modal */}
      <Composer />
    </div>
  );
}
