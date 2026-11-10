// The split view's behaviour: the two panes' ids live in the URL, and each action rewrites one pane's token.
import { useCallback, useMemo } from 'react';
import type { TabInfo } from '../components/layout/TopBar';
import { isRouteTab, splitPath, tabPath, type Route } from '../lib/route';
import { usePaneTabs } from './usePaneTabs';
import type { useTabs } from './useTabs';

type Side = 'left' | 'right';

export function useSplitView(
  route: Route,
  navigate: (path: string) => void,
  { tabs, setTabs, activeTabId, setActiveTabId }: ReturnType<typeof useTabs>,
  tabInfo: (id: string) => TabInfo,
  pickTab: (id: string) => void,
  adoptSession: (newId: string, pseudoId: string | null) => void,
  refreshProjects: () => void
) {
  const { paneTabs, closeTab: closePaneTabIn, replaceTab: replacePaneTab } = usePaneTabs(
    route.name === 'split' ? { left: route.left, right: route.right } : null,
    tabs
  );
  const paneTabInfos = useMemo(
    () => ({ left: (paneTabs?.left ?? []).map(tabInfo), right: (paneTabs?.right ?? []).map(tabInfo) }),
    [paneTabs, tabInfo]
  );

  // The split path with one pane's token replaced.
  const withPane = useCallback(
    (side: Side, token: string | null) => {
      const { left, right } = route.name === 'split' ? route : { left: null, right: null };
      return splitPath(side === 'left' ? token : left, side === 'right' ? token : right);
    },
    [route]
  );

  const openSessionInPane = useCallback(
    (side: Side, sessionId: string) => {
      setTabs((previous) => (previous.includes(sessionId) ? previous : [...previous, sessionId]));
      navigate(withPane(side, sessionId));
    },
    [withPane, navigate]
  );

  const newSessionInPane = useCallback(
    (side: Side, projectId: string) => navigate(withPane(side, `new:${projectId}`)),
    [withPane, navigate]
  );

  const sessionCreatedInPane = useCallback(
    (side: Side, newId: string, pseudoId: string | null) => {
      if (pseudoId) replacePaneTab(pseudoId, newId);
      adoptSession(newId, pseudoId);
      navigate(withPane(side, newId));
      refreshProjects();
    },
    [adoptSession, withPane, navigate, refreshProjects, replacePaneTab]
  );

  const toggleSplit = useCallback(() => {
    if (route.name === 'split') {
      // Exit to a single session view: the left pane, else the active tab.
      const target = route.left ?? activeTabId;
      navigate(target ? tabPath(target) : '#/');
      return;
    }
    const current =
      route.name === 'session' ? route.id : route.name === 'new' ? `new:${route.id}` : activeTabId;
    navigate(splitPath(current, null));
  }, [route, activeTabId, navigate]);

  const closePane = useCallback(
    (side: Side) => {
      if (route.name !== 'split') return;
      const other = side === 'left' ? route.right : route.left;
      navigate(other ? tabPath(other) : '#/');
    },
    [route, navigate]
  );

  const pickTabInPane = useCallback(
    (side: Side, id: string) => {
      if (route.name !== 'split' || isRouteTab(id)) {
        pickTab(id);
        return;
      }
      setActiveTabId(id);
      navigate(withPane(side, id));
    },
    [route, pickTab, navigate, withPane]
  );

  const closeTabInPane = useCallback(
    (side: Side, id: string) => {
      const fallback = closePaneTabIn(side, id);
      if (route.name !== 'split' || route[side] !== id) return;
      navigate(withPane(side, fallback));
    },
    [closePaneTabIn, route, navigate, withPane]
  );

  return {
    paneTabInfos,
    openSessionInPane,
    newSessionInPane,
    sessionCreatedInPane,
    toggleSplit,
    closePane,
    pickTabInPane,
    closeTabInPane
  };
}
