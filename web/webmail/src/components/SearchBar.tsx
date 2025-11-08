'use client';

import { useState, useEffect } from 'react';
import { useMailStore } from '@/store/mail-store';
import { jmapClient } from '@/lib/jmap-client';
import { MagnifyingGlassIcon } from '@heroicons/react/24/outline';
import { useDebouncedValue } from '@/hooks/use-debounced-value';

export function SearchBar() {
  const {
    searchQuery,
    setSearchQuery,
    setSearchResults,
    setIsSearching,
    setEmails,
  } = useMailStore();
  const [localQuery, setLocalQuery] = useState(searchQuery);
  const debouncedQuery = useDebouncedValue(localQuery, 300);

  useEffect(() => {
    if (debouncedQuery) {
      performSearch(debouncedQuery);
    } else {
      setSearchResults([]);
      setSearchQuery('');
    }
  }, [debouncedQuery]);

  async function performSearch(query: string) {
    if (!query.trim()) return;

    setIsSearching(true);
    setSearchQuery(query);

    try {
      const { emails } = await jmapClient.searchEmails(query);
      setSearchResults(emails);
      setEmails(emails); // Show search results in email list
    } catch (error) {
      console.error('Search failed:', error);
    } finally {
      setIsSearching(false);
    }
  }

  return (
    <div className="relative flex-1 max-w-2xl">
      <div className="relative">
        <MagnifyingGlassIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" />
        <input
          type="text"
          value={localQuery}
          onChange={(e) => setLocalQuery(e.target.value)}
          placeholder="Search mail..."
          className="w-full pl-10 pr-4 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
      </div>
    </div>
  );
}
