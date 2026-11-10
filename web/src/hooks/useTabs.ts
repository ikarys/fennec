import { useCallback, useEffect, useState } from 'react';
import { ACTIVE_TAB_KEY, TABS_KEY, isRouteTab, loadActiveTab, loadTabs } from '../lib/route';
import { writeStored } from '../lib/storage';

export function useTabs() {
  const [tabs, setTabs] = useState<string[]>(() => loadTabs());
  const [activeTabId, setActiveTabId] = useState<string | null>(() => loadActiveTab());

  useEffect(() => {
    // Only session tabs are persisted: route tabs (pages, editors) are re-created
    // by the route-sync effect from the URL on load, so a one-time visit to a
    // page or editor must not leave a permanent tab in the bar.
    writeStored(TABS_KEY, JSON.stringify(tabs.filter((id) => !isRouteTab(id))));
  }, [tabs]);

  useEffect(() => {
    writeStored(ACTIVE_TAB_KEY, activeTabId || null);
  }, [activeTabId]);

  const openTab = useCallback((id: string) => {
    setTabs((previous) => (previous.includes(id) ? previous : [...previous, id]));
    setActiveTabId(id);
  }, []);

  return { tabs, setTabs, activeTabId, setActiveTabId, openTab };
}
