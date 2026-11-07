import { useCallback, useEffect, useState } from 'react';
import { ACTIVE_TAB_KEY, TABS_KEY, loadActiveTab, loadTabs } from '../lib/route';

export function useTabs() {
  const [tabs, setTabs] = useState<string[]>(() => loadTabs());
  const [activeTabId, setActiveTabId] = useState<string | null>(() => loadActiveTab());

  useEffect(() => {
    // Only session tabs are persisted: page tabs are re-created by the
    // route-sync effect from the URL on load, so a one-time visit to a page
    // must not leave a permanent tab in the bar.
    try {
      localStorage.setItem(TABS_KEY, JSON.stringify(tabs.filter((id) => !id.startsWith('page:'))));
    } catch {
      // Ignore storage errors.
    }
  }, [tabs]);

  useEffect(() => {
    try {
      if (activeTabId) {
        localStorage.setItem(ACTIVE_TAB_KEY, activeTabId);
      } else {
        localStorage.removeItem(ACTIVE_TAB_KEY);
      }
    } catch {
      // Ignore storage errors.
    }
  }, [activeTabId]);

  const openTab = useCallback((id: string) => {
    setTabs((previous) => (previous.includes(id) ? previous : [...previous, id]));
    setActiveTabId(id);
  }, []);

  return { tabs, setTabs, activeTabId, setActiveTabId, openTab };
}
