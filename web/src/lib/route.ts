// Routes and tabs: the hash read as a route, a tab's path, what localStorage keeps of the tabs.
// Pure, so the parsing is tested without a browser.
import type { MsgKey } from '../i18n/messages';

/** The fixed pages a tab can show. */
export type PageId = 'projects' | 'chats' | 'settings' | 'insight' | 'workflows' | 'backlog';

export type Route =
  | { name: 'home' }
  | { name: 'session'; id: string }
  | { name: 'changes'; id: string }
  | { name: 'new'; id: string; testWorkflowId: string | null; frameIdeaId: string | null; fromSessionId: string | null }
  | { name: 'settings'; tab: string | null }
  | { name: 'insight' }
  | { name: 'backlog' }
  | { name: 'projects' }
  | { name: 'chats' }
  | { name: 'split'; left: string | null; right: string | null }
  | { name: 'project'; projectId: string; tab: string | null; sub: string | null }
  | { name: 'editor'; projectId: string; path: string | null; session: string | null }
  | { name: 'workflows'; projectId: string | null; workflowId: string | null; isNew: boolean };

export const TABS_KEY = 'fennec.tabs';
export const ACTIVE_TAB_KEY = 'fennec.activeTab';

/** The fixed pages that get a tab in the bar, in display order. */
export const PAGE_TABS: { page: PageId; titleKey: MsgKey; path: string }[] = [
  { page: 'projects', titleKey: 'paletteProjects', path: '#/projects' },
  { page: 'chats', titleKey: 'homeChat', path: '#/chats' },
  { page: 'settings', titleKey: 'settings', path: '#/settings' },
  { page: 'insight', titleKey: 'insight', path: '#/insight' },
  { page: 'workflows', titleKey: 'workflows', path: '#/workflows' },
  { page: 'backlog', titleKey: 'tasks', path: '#/backlog' }
];

export function parseRoute(hash: string): Route {
  if (hash === '#/settings' || hash.startsWith('#/settings/')) {
    return { name: 'settings', tab: hash.slice('#/settings/'.length) || null };
  }
  if (hash === '#/insight') {
    return { name: 'insight' };
  }
  if (hash === '#/backlog') {
    return { name: 'backlog' };
  }
  if (hash === '#/projects') {
    return { name: 'projects' };
  }
  if (hash === '#/chats') {
    return { name: 'chats' };
  }
  if (hash.startsWith('#/project/')) {
    const [rawId, tab, sub] = hash.slice('#/project/'.length).split('/');
    return { name: 'project', projectId: decodeURIComponent(rawId), tab: tab ?? null, sub: sub ?? null };
  }
  if (hash.startsWith('#/edit/')) {
    const [pathPart, query] = hash.slice('#/edit/'.length).split('?');
    const [rawId, rawPath] = (pathPart ?? '').split('/');
    const session = query ? new URLSearchParams(query).get('session') : null;
    return {
      name: 'editor',
      projectId: decodeURIComponent(rawId ?? ''),
      path: rawPath ? decodeURIComponent(rawPath) : null,
      session,
    };
  }
  if (hash.startsWith('#/workflows')) {
    const path = hash.slice(12); // Remove '#/workflows'
    const segments = path.split('/').filter((s) => s.length > 0);
    let isNew = false;
    if (segments.length > 0 && segments[segments.length - 1].includes('?new')) {
      isNew = true;
      segments[segments.length - 1] = segments[segments.length - 1].split('?')[0];
    }
    const projectId = segments.length >= 1 ? decodeURIComponent(segments[0]) : null;
    const workflowId = segments.length >= 2 ? decodeURIComponent(segments[1]) : null;
    return { name: 'workflows', projectId, workflowId, isNew };
  }
  if (hash.startsWith('#/split')) {
    const rest = hash.slice('#/split'.length).replace(/^\//, '');
    const parts = rest.split('/');
    return { name: 'split', left: parsePaneToken(parts[0] ?? null), right: parsePaneToken(parts[1] ?? null) };
  }
  if (hash.startsWith('#/s/')) {
    const [rawId, sub] = hash.slice('#/s/'.length).split('/');
    if (sub === 'changes') {
      return { name: 'changes', id: decodeURIComponent(rawId) };
    }
    const id = decodeURIComponent(rawId);
    if (id.startsWith('new:')) {
      return { name: 'new', id: id.slice(4), testWorkflowId: null, frameIdeaId: null, fromSessionId: null };
    }
    return { name: 'session', id };
  }
  if (hash.startsWith('#/new/')) {
    const [rawId, query] = hash.slice(6).split('?');
    const testWorkflowId = query ? new URLSearchParams(query).get('test') : null;
    const frameIdeaId = query ? new URLSearchParams(query).get('frame') : null;
    const fromSessionId = query ? new URLSearchParams(query).get('from') : null;
    return { name: 'new', id: decodeURIComponent(rawId), testWorkflowId, frameIdeaId, fromSessionId };
  }
  return { name: 'home' };
}

/** The route of a tab: a pseudo `new:` tab opens the new-session page, a page tab its page. */
export function tabPath(tabId: string): string {
  if (tabId.startsWith('new:')) {
    return `#/new/${encodeURIComponent(tabId.slice(4))}`;
  }
  if (tabId.startsWith('page:')) {
    return PAGE_TABS.find((page) => page.page === tabId.slice(5))?.path ?? '#/';
  }
  return `#/s/${encodeURIComponent(tabId)}`;
}

/** The changes page of a session. */
export function changesPath(sessionId: string): string {
  return `#/s/${encodeURIComponent(sessionId)}/changes`;
}

/** The editor page for a project's files; optional file path and session id. */
export function editorPath(projectId: string, path: string | null, sessionId: string | null = null): string {
  return (
    `#/edit/${encodeURIComponent(projectId)}` +
    (path ? `/${encodeURIComponent(path)}` : '') +
    (sessionId ? `?session=${encodeURIComponent(sessionId)}` : '')
  );
}

/** The tab id on screen, from the route; null on pages that have no tab (home, project, split). */
export function tabIdForRoute(route: Route): string | null {
  switch (route.name) {
    case 'session':
    case 'changes':
      return route.id;
    case 'new':
      return `new:${route.id}`;
    case 'settings':
      return 'page:settings';
    case 'insight':
      return 'page:insight';
    case 'backlog':
      return 'page:backlog';
    case 'workflows':
      return 'page:workflows';
    case 'projects':
      return 'page:projects';
    case 'chats':
      return 'page:chats';
    case 'editor':
      return null;
    default:
      return null;
  }
}

/** A split pane's value in the URL: a session id, `new:<projectId>`, or `-` for empty. */
export function paneToken(value: string | null): string {
  return value ? encodeURIComponent(value) : '-';
}

export function splitPath(left: string | null, right: string | null): string {
  return `#/split/${paneToken(left)}/${paneToken(right)}`;
}

export function parsePaneToken(raw: string | null | undefined): string | null {
  if (!raw || raw === '-') return null;
  return decodeURIComponent(raw);
}

export function loadTabs(): string[] {
  try {
    const raw = localStorage.getItem(TABS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string => typeof value === 'string' && !value.startsWith('page:'))
      : [];
  } catch {
    return [];
  }
}

export function loadActiveTab(): string | null {
  try {
    const id = localStorage.getItem(ACTIVE_TAB_KEY);
    // A page tab persisted by an older build is not a real session.
    return id && !id.startsWith('page:') ? id : null;
  } catch {
    return null;
  }
}
