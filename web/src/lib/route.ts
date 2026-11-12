// Routes and tabs: the hash read as a route, a tab's path, what localStorage keeps of the tabs.
// Pure, so the parsing is tested without a browser.
import type { MsgKey } from '../i18n/messages';
import { readStored } from './storage';

/** The fixed pages a tab can show. */
export type PageId = 'projects' | 'chats' | 'settings' | 'insight' | 'workflows' | 'backlog';

/** A workflow opened from the editor: preselected in the new session, step by step (test) or not (run). */
export interface LaunchWorkflow { id: string; stepByStep: boolean }

export type Route =
  | { name: 'home' }
  | { name: 'session'; id: string }
  | { name: 'changes'; id: string; path: string | null }
  | { name: 'new'; id: string; launchWorkflow: LaunchWorkflow | null; frameIdeaId: string | null; fromSessionId: string | null }
  | { name: 'settings'; tab: string | null }
  | { name: 'insight' }
  | { name: 'backlog' }
  | { name: 'projects' }
  | { name: 'chats' }
  | { name: 'split'; left: string | null; right: string | null }
  | { name: 'project'; projectId: string; tab: string | null; sub: string | null; task: string | null }
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
    const [pathPart, query] = hash.slice('#/project/'.length).split('?');
    const [rawId, tab, sub] = (pathPart ?? '').split('/');
    const task = query ? new URLSearchParams(query).get('task') : null;
    return { name: 'project', projectId: decodeURIComponent(rawId), tab: tab ?? null, sub: sub ?? null, task };
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
    const [rawId, sub, rawPath] = hash.slice('#/s/'.length).split('/');
    if (sub === 'changes') {
      return { name: 'changes', id: decodeURIComponent(rawId), path: rawPath ? decodeURIComponent(rawPath) : null };
    }
    const id = decodeURIComponent(rawId);
    if (id.startsWith('new:')) {
      return { name: 'new', id: id.slice(4), launchWorkflow: null, frameIdeaId: null, fromSessionId: null };
    }
    return { name: 'session', id };
  }
  if (hash.startsWith('#/new/')) {
    const [rawId, query] = hash.slice(6).split('?');
    const params = query ? new URLSearchParams(query) : null;
    const testWorkflowId = params?.get('test') || null;
    const runWorkflowId = params?.get('run') || null;
    const launchWorkflow = testWorkflowId
      ? { id: testWorkflowId, stepByStep: true }
      : runWorkflowId
        ? { id: runWorkflowId, stepByStep: false }
        : null;
    const frameIdeaId = params?.get('frame') || null;
    const fromSessionId = params?.get('from') || null;
    return { name: 'new', id: decodeURIComponent(rawId), launchWorkflow, frameIdeaId, fromSessionId };
  }
  return { name: 'home' };
}

/** A tab derived from the route (a page, a project's editor), never persisted nor split. */
/** The same hash with its query replaced by the task open in the drawer, or none. */
export function withTask(hash: string, ideaId: string | null): string {
  const path = hash.split('?')[0];
  return ideaId === null ? path : `${path}?task=${encodeURIComponent(ideaId)}`;
}

export function isRouteTab(id: string): boolean {
  return id.startsWith('page:') || id.startsWith('edit:');
}

/** The tab of a project's editor. */
export function editorTabId(projectId: string): string {
  return `edit:${projectId}`;
}

const lastEditorHash = new Map<string, string>();

/** Remembers where a project's editor is, so its tab returns there. */
export function rememberEditorHash(projectId: string, hash: string): void {
  lastEditorHash.set(projectId, hash);
}

/** The route of a tab: a pseudo `new:` tab opens the new-session page, a page tab its page, an editor tab its last file. */
export function tabPath(tabId: string): string {
  if (tabId.startsWith('new:')) {
    return `#/new/${encodeURIComponent(tabId.slice(4))}`;
  }
  if (tabId.startsWith('page:')) {
    return PAGE_TABS.find((page) => page.page === tabId.slice(5))?.path ?? '#/';
  }
  if (tabId.startsWith('edit:')) {
    return lastEditorHash.get(tabId.slice(5)) ?? editorPath(tabId.slice(5), null);
  }
  return `#/s/${encodeURIComponent(tabId)}`;
}

/** The changes page of a session, optionally focused on one file. */
export function changesPath(sessionId: string, path: string | null = null): string {
  return `#/s/${encodeURIComponent(sessionId)}/changes` + (path ? `/${encodeURIComponent(path)}` : '');
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
      return editorTabId(route.projectId);
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
    const raw = readStored(TABS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string => typeof value === 'string' && !isRouteTab(value))
      : [];
  } catch {
    return [];
  }
}

export function loadActiveTab(): string | null {
  const id = readStored(ACTIVE_TAB_KEY);
  // A route tab persisted by an older build is not a real session.
  return id && !isRouteTab(id) ? id : null;
}
