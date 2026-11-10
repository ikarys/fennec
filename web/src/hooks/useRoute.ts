// The current hash route and how to move: navigation, plus the tab a page or editor route gets in the bar.
import { useCallback, useEffect, useState } from 'react';
import { editorTabId, parseRoute, rememberEditorHash, type Route } from '../lib/route';

const PAGE_ROUTES = ['settings', 'insight', 'backlog', 'workflows', 'projects', 'chats'];

export function useRoute(openTab: (id: string) => void) {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash));

  useEffect(() => {
    const onHashChange = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, []);

  const navigate = useCallback((path: string) => {
    if (window.location.hash !== path) {
      window.location.hash = path;
    } else {
      setRoute(parseRoute(window.location.hash));
    }
  }, []);

  // A route-derived tab (page, project editor) gets its tab in the bar the
  // moment it is on screen, so the active route is never unlisted.
  const pageTabIdForRoute =
    route.name === 'editor'
      ? editorTabId(route.projectId)
      : PAGE_ROUTES.includes(route.name)
        ? `page:${route.name}`
        : null;
  useEffect(() => {
    if (!pageTabIdForRoute) return;
    openTab(pageTabIdForRoute);
  }, [pageTabIdForRoute, openTab]);

  // The editor's last file/session location is remembered per project, so the
  // editor tab can return there when picked again.
  useEffect(() => {
    if (route.name === 'editor') {
      rememberEditorHash(route.projectId, window.location.hash);
    }
  }, [route]);

  return { route, navigate };
}
