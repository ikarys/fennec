// The tab bar's behaviour: how a tab looks, which one is picked or closed, and how a created session takes its tab.
import { useCallback, useMemo } from 'react';
import type { TabInfo } from '../components/layout/TopBar';
import type { Project } from '../lib/api';
import { PAGE_TABS, isRouteTab, tabIdForRoute, tabPath, type Route } from '../lib/route';
import { t } from '../i18n/messages';
import type { Lang } from '../i18n/messages';
import type { useTabs } from './useTabs';

type Tabs = ReturnType<typeof useTabs>;

// Re-clicking the tab of the page already on screen keeps its sub-tab
// (settings tab, project id, …) instead of resetting to the page's root.
const basePathOf = (hash: string): string => {
  const segments = hash.slice(1).split('/').filter((segment) => segment.length > 0);
  return `#/${segments.slice(0, 2).join('/')}`;
};

export function useTabNavigation(
  { tabs, setTabs, activeTabId, setActiveTabId }: Tabs,
  route: Route,
  navigate: (path: string) => void,
  projects: Project[],
  language: Lang,
  refreshProjects: () => void
) {
  const tabInfo = useCallback((id: string): TabInfo => {
    if (id.startsWith('edit:')) {
      const projectId = id.slice(5);
      const name = projects.find((candidate) => candidate.id === projectId)?.name ?? projectId;
      return { id, title: t(language, 'editorTabTitle', { project: name }), kind: 'editor' as const, project: name };
    }
    if (id.startsWith('page:')) {
      const page = PAGE_TABS.find((candidate) => candidate.page === id.slice(5));
      return { id, title: page ? t(language, page.titleKey) : t(language, 'page'), kind: 'page', page: page?.page ?? 'settings' };
    }
    if (id.startsWith('new:')) {
      const projectId = id.slice(4);
      const project = projects.find((candidate) => candidate.id === projectId);
      return {
        id,
        title: t(language, project?.chat ? 'paletteNewChat' : 'newSession'),
        project: project?.name ?? projectId,
        state: 'idle' as const,
        kind: 'session' as const
      };
    }
    for (const project of projects) {
      const session = project.sessions.find((candidate) => candidate.id === id);
      if (session) {
        return { id, title: session.title, project: project.name, state: session.state, kind: 'session' as const };
      }
    }
    return { id, title: t(language, 'session'), project: '—', state: 'idle' as const, kind: 'session' as const };
  }, [projects, language]);
  const tabInfos = useMemo(() => tabs.map(tabInfo), [tabs, tabInfo]);

  const closeTab = useCallback(
    (tabId: string) => {
      const next = tabs.filter((id) => id !== tabId);
      setTabs(next);
      // Only the tab that is on screen (per the route) triggers a navigation.
      // activeTabId can be stale when a session was opened by URL/bookmark, so
      // it is not a reliable "is this tab showing" signal.
      if (tabIdForRoute(route) !== tabId) return;
      // A route tab closing while active returns home; a session tab falls back
      // to the last remaining session tab, or home when none are left. Route
      // tabs are re-derived from the route, so they are not a close target.
      if (isRouteTab(tabId)) {
        setActiveTabId(null);
        navigate('#/');
        return;
      }
      const fallback = next.filter((id) => !isRouteTab(id)).pop() ?? null;
      setActiveTabId(fallback);
      navigate(fallback ? tabPath(fallback) : '#/');
    },
    [tabs, route, navigate]
  );

  const pickTab = useCallback(
    (id: string) => {
      setActiveTabId(id);
      const target = tabPath(id);
      if (!isRouteTab(id) || basePathOf(window.location.hash) !== target) {
        navigate(target);
      }
    },
    [navigate]
  );

  const handleSessionsDeleted = useCallback(
    (ids: string[]) => {
      const deleted = new Set(ids);
      setTabs((previous) => previous.filter((id) => !deleted.has(id)));
      if (activeTabId !== null && deleted.has(activeTabId)) {
        setActiveTabId(null);
      }
      refreshProjects();
    },
    [activeTabId, refreshProjects]
  );

  // A created session takes the tab of its draft (`new:…`), or a tab of its own.
  const adoptSession = useCallback(
    (newId: string, pseudoId: string | null) => {
      setTabs((previous) => {
        const replaced = pseudoId ? previous.map((id) => (id === pseudoId ? newId : id)) : previous;
        return replaced.includes(newId) ? replaced : [...replaced, newId];
      });
      if (activeTabId === pseudoId) {
        setActiveTabId(newId);
      }
    },
    [activeTabId]
  );

  return { tabInfo, tabInfos, closeTab, pickTab, handleSessionsDeleted, adoptSession };
}
