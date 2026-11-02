import { useCallback, useEffect, useMemo, useRef, useState, lazy, Suspense } from 'react';
import type { Project, ProviderListing, SettingsResponse } from './lib/api';
import { createProject, getProjects, getSettings } from './lib/api';
import { applyTheme, getStoredTheme, type ThemeId } from './lib/theme';
import { TopBar, type TabInfo } from './components/layout/TopBar';
import { HomePage } from './pages/HomePage';
import { SessionPage } from './pages/SessionPage';
import { SplitPage } from './pages/SplitPage';
import { SettingsPage } from './pages/SettingsPage';
import { InsightPage } from './pages/InsightPage';
import { Palette } from './components/Palette';
import { ProjectDialog } from './components/project/ProjectDialog';
import { DisplayPrefsProvider, defaultDisplayPrefs } from './lib/display';
import { useTurnNotices } from './hooks/useTurnNotices';

const WorkflowsPage = lazy(() => import('./pages/WorkflowsPage'));
const ProjectPage = lazy(() => import('./pages/ProjectPage'));
const BacklogPage = lazy(() => import('./pages/BacklogPage'));

type Route =
  | { name: 'home' }
  | { name: 'session'; id: string }
  | { name: 'new'; id: string; testWorkflowId: string | null; frameIdeaId: string | null; fromSessionId: string | null }
  | { name: 'settings'; tab: string | null }
  | { name: 'insight' }
  | { name: 'backlog' }
  | { name: 'split'; left: string | null; right: string | null }
  | { name: 'project'; projectId: string; tab: string | null; sub: string | null }
  | { name: 'workflows'; projectId: string | null; workflowId: string | null; isNew: boolean };

const TABS_KEY = 'fennec.tabs';
const ACTIVE_TAB_KEY = 'fennec.activeTab';

function parseRoute(): Route {
  const hash = window.location.hash;
  if (hash === '#/settings' || hash.startsWith('#/settings/')) {
    return { name: 'settings', tab: hash.slice('#/settings/'.length) || null };
  }
  if (hash === '#/insight') {
    return { name: 'insight' };
  }
  if (hash === '#/backlog') {
    return { name: 'backlog' };
  }
  if (hash.startsWith('#/project/')) {
    const [rawId, tab, sub] = hash.slice('#/project/'.length).split('/');
    return { name: 'project', projectId: decodeURIComponent(rawId), tab: tab ?? null, sub: sub ?? null };
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
    const id = decodeURIComponent(hash.slice(4));
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

/** The route of a tab: a pseudo `new:` tab opens the new-session page. */
function tabPath(tabId: string): string {
  return tabId.startsWith('new:')
    ? `#/new/${encodeURIComponent(tabId.slice(4))}`
    : `#/s/${encodeURIComponent(tabId)}`;
}

/** A split pane's value in the URL: a session id, `new:<projectId>`, or `-` for empty. */
function paneToken(value: string | null): string {
  return value ? encodeURIComponent(value) : '-';
}

function splitPath(left: string | null, right: string | null): string {
  return `#/split/${paneToken(left)}/${paneToken(right)}`;
}

function parsePaneToken(raw: string | null | undefined): string | null {
  if (!raw || raw === '-') return null;
  return decodeURIComponent(raw);
}

function loadTabs(): string[] {
  try {
    const raw = localStorage.getItem(TABS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === 'string') : [];
  } catch {
    return [];
  }
}

function loadActiveTab(): string | null {
  try {
    return localStorage.getItem(ACTIVE_TAB_KEY);
  } catch {
    return null;
  }
}

export default function App() {
  const [route, setRoute] = useState<Route>(() => parseRoute());
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);
  const [projectsError, setProjectsError] = useState<string | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [tabs, setTabs] = useState<string[]>(() => loadTabs());
  const [activeTabId, setActiveTabId] = useState<string | null>(() => loadActiveTab());
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [projectDialogOpen, setProjectDialogOpen] = useState(false);
  const [projectSubmitting, setProjectSubmitting] = useState(false);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemeId>(() => getStoredTheme());
  // The ⌘\ handler is attached once; it always calls the latest toggle.
  const toggleSplitRef = useRef<() => void>(() => {});

  useEffect(() => {
    const onHashChange = () => setRoute(parseRoute());
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, []);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    try {
      localStorage.setItem(TABS_KEY, JSON.stringify(tabs));
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

  // `projectsLoading` is only the first load: a refresh keeps the list on
  // screen instead of flashing a loading state.
  const refreshProjects = useCallback(async () => {
    try {
      const data = await getProjects();
      setProjects(data);
      setProjectsError(null);
    } catch (error) {
      setProjectsError(error instanceof Error ? error.message : 'Impossible de charger les projets');
    } finally {
      setProjectsLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshProjects();
  }, [refreshProjects]);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch(() => setSettings(null));
  }, []);

  const handleSettingsSaved = useCallback((next: SettingsResponse) => {
    setSettings(next);
  }, []);

  // A provider listing refreshed on demand (the fetch button) replaces that
  // provider's entry, or appends it when the provider was not listed yet.
  const handleListing = useCallback((listing: ProviderListing) => {
    setSettings((previous) => {
      if (!previous) return previous;
      const exists = previous.listings.some((item) => item.provider === listing.provider);
      const listings = exists
        ? previous.listings.map((item) => (item.provider === listing.provider ? listing : item))
        : [...previous.listings, listing];
      return { ...previous, listings };
    });
  }, []);

  // No polling: the list refreshes when the human looks at it (home, project,
  // palette) and when a turn starts or ends; F5 does the rest.
  const projectsVisible =
    route.name === 'home' || route.name === 'project' || paletteOpen;
  useEffect(() => {
    if (projectsVisible) refreshProjects();
  }, [projectsVisible, refreshProjects]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
      if ((event.metaKey || event.ctrlKey) && event.key === '\\') {
        event.preventDefault();
        toggleSplitRef.current();
      }
      if (event.key === 'Escape') {
        setPaletteOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const displayPrefs = settings?.preferences.display ?? defaultDisplayPrefs;

  // The chosen size scales the text only: every text size is in rem, set
  // from the root font size (16px for the default 14px body), while the
  // layout's spacing stays in px (styles/app.css).
  useEffect(() => {
    document.documentElement.style.fontSize = `${(16 * displayPrefs.fontSize) / defaultDisplayPrefs.fontSize}px`;
  }, [displayPrefs.fontSize]);

  const navigate = useCallback((path: string) => {
    if (window.location.hash !== path) {
      window.location.hash = path;
    } else {
      setRoute(parseRoute());
    }
  }, []);

  const tabInfos = useMemo<TabInfo[]>(() => {
    return tabs.map((id) => {
      if (id.startsWith('new:')) {
        const projectId = id.slice(4);
        const project = projects.find((candidate) => candidate.id === projectId);
        return {
          id,
          title: 'Nouvelle session',
          project: project?.name ?? projectId,
          state: 'idle' as const
        };
      }
      for (const project of projects) {
        const session = project.sessions.find((candidate) => candidate.id === id);
        if (session) {
          return { id, title: session.title, project: project.name, state: session.state };
        }
      }
      return { id, title: 'Session', project: '—', state: 'idle' as const };
    });
  }, [tabs, projects]);

  const openSession = useCallback(
    (sessionId: string) => {
      setTabs((previous) => (previous.includes(sessionId) ? previous : [...previous, sessionId]));
      setActiveTabId(sessionId);
      navigate(`#/s/${encodeURIComponent(sessionId)}`);
    },
    [navigate]
  );
  const { items: attention } = useTurnNotices(openSession);

  const closeTab = useCallback(
    (tabId: string) => {
      setTabs((previous) => {
        const next = previous.filter((id) => id !== tabId);
        if (activeTabId === tabId) {
          const fallback = next.length > 0 ? next[next.length - 1] : null;
          setActiveTabId(fallback);
          if (fallback) {
            navigate(tabPath(fallback));
          } else {
            navigate('#/');
          }
        }
        return next;
      });
    },
    [activeTabId, navigate]
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

  const startNewSession = useCallback(
    (projectId?: string) => {
      const targetProjectId = projectId ?? projects[0]?.id;
      if (!targetProjectId) {
        setProjectDialogOpen(true);
        return;
      }
      const pseudoId = `new:${targetProjectId}`;
      setTabs((previous) => (previous.includes(pseudoId) ? previous : [...previous, pseudoId]));
      setActiveTabId(pseudoId);
      navigate(`#/new/${encodeURIComponent(targetProjectId)}`);
    },
    [navigate, projects]
  );

  const handleSessionCreated = useCallback(
    (newId: string, pseudoId: string | null) => {
      setTabs((previous) => {
        const replaced = pseudoId ? previous.map((id) => (id === pseudoId ? newId : id)) : previous;
        return replaced.includes(newId) ? replaced : [...replaced, newId];
      });
      if (activeTabId === pseudoId) {
        setActiveTabId(newId);
      }
      navigate(`#/s/${encodeURIComponent(newId)}`);
      refreshProjects();
    },
    [activeTabId, navigate, refreshProjects]
  );

  const handleCreateProject = useCallback(
    async (workdir: string) => {
      setProjectSubmitting(true);
      setProjectError(null);
      try {
        await createProject(workdir);
        setProjectDialogOpen(false);
        await refreshProjects();
      } catch (error) {
        setProjectError(error instanceof Error ? error.message : 'Impossible d’ouvrir le projet');
      } finally {
        setProjectSubmitting(false);
      }
    },
    [refreshProjects]
  );

  // Split view: the two panes' ids live in the URL; each pane is an independent
  // SessionPage. Choosing a session or creating one rewrites that pane's token.
  const openSessionInPane = useCallback(
    (side: 'left' | 'right', sessionId: string) => {
      setTabs((previous) => (previous.includes(sessionId) ? previous : [...previous, sessionId]));
      const { left, right } = route.name === 'split' ? route : { left: null, right: null };
      navigate(splitPath(side === 'left' ? sessionId : left, side === 'right' ? sessionId : right));
    },
    [route, navigate]
  );

  const newSessionInPane = useCallback(
    (side: 'left' | 'right', projectId: string) => {
      const { left, right } = route.name === 'split' ? route : { left: null, right: null };
      navigate(splitPath(side === 'left' ? `new:${projectId}` : left, side === 'right' ? `new:${projectId}` : right));
    },
    [route, navigate]
  );

  const sessionCreatedInPane = useCallback(
    (side: 'left' | 'right', newId: string, pseudoId: string | null) => {
      setTabs((previous) => {
        const replaced = pseudoId ? previous.map((id) => (id === pseudoId ? newId : id)) : previous;
        return replaced.includes(newId) ? replaced : [...replaced, newId];
      });
      if (activeTabId === pseudoId) setActiveTabId(newId);
      const { left, right } = route.name === 'split' ? route : { left: null, right: null };
      navigate(splitPath(side === 'left' ? newId : left, side === 'right' ? newId : right));
      refreshProjects();
    },
    [activeTabId, route, navigate, refreshProjects]
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

  useEffect(() => {
    toggleSplitRef.current = toggleSplit;
  }, [toggleSplit]);

  const pseudoTabId = route.name === 'new' ? `new:${route.id}` : null;

  return (
    <DisplayPrefsProvider value={displayPrefs}>
      {/* h-full: the html/body/#root chain is 100%, which no page-level
          scaling can push past the window. */}
      <div className="flex h-full flex-col overflow-hidden bg-bg text-text">
        <TopBar
          tabs={tabInfos}
          activeTabId={activeTabId}
          attention={attention}
          onOpenAttention={openSession}
          onPickTab={(id) => {
            setActiveTabId(id);
            navigate(tabPath(id));
          }}
          onCloseTab={closeTab}
          onOpenPalette={() => setPaletteOpen(true)}
          onNewSession={() => startNewSession()}
          splitActive={route.name === 'split'}
          onToggleSplit={toggleSplit}
        />

        <div className="flex min-h-0 grow">
          {route.name === 'home' ? (
            <HomePage
              projects={projects}
              loading={projectsLoading}
              error={projectsError}
              onOpenProjectDialog={() => setProjectDialogOpen(true)}
              onNewSession={(projectId) => startNewSession(projectId)}
              onOpenSession={openSession}
              onSessionsDeleted={handleSessionsDeleted}
            />
          ) : route.name === 'settings' ? (
            <SettingsPage
              key={route.tab ?? ''}
              initialTab={route.tab}
              settings={settings}
              theme={theme}
              onThemeChange={setTheme}
              onSaved={handleSettingsSaved}
              onListing={handleListing}
            />
          ) : route.name === 'workflows' ? (
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">Chargement…</div>}>
              <WorkflowsPage
                projects={projects}
                projectId={route.projectId}
                workflowId={route.workflowId}
                isNew={route.isNew}
              />
            </Suspense>
          ) : route.name === 'project' ? (
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">Chargement…</div>}>
              <ProjectPage projects={projects} projectId={route.projectId} tab={route.tab} sub={route.sub} onOpenSession={openSession} />
            </Suspense>
          ) : route.name === 'insight' ? (
            <InsightPage />
          ) : route.name === 'backlog' ? (
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">Chargement…</div>}>
              <BacklogPage />
            </Suspense>
          ) : route.name === 'split' ? (
            <SplitPage
              left={route.left}
              right={route.right}
              projects={projects}
              onChooseSession={openSessionInPane}
              onNewSessionInPane={newSessionInPane}
              onSessionCreated={sessionCreatedInPane}
              onRunningChanged={refreshProjects}
              onRenamed={refreshProjects}
              onOpenProjectDialog={() => setProjectDialogOpen(true)}
              onHome={() => navigate('#/')}
            />
          ) : (
            <SessionPage
              sessionId={route.name === 'session' ? route.id : null}
              projectId={route.name === 'new' ? route.id : null}
              testWorkflowId={route.name === 'new' ? route.testWorkflowId : null}
              frameIdeaId={route.name === 'new' ? route.frameIdeaId : null}
              fromSessionId={route.name === 'new' ? route.fromSessionId : null}
              pseudoTabId={pseudoTabId}
              projects={projects}
              onSessionCreated={handleSessionCreated}
              onRunningChanged={refreshProjects}
              onRenamed={refreshProjects}
            />
          )}
        </div>

        <Palette
          open={paletteOpen}
          projects={projects}
          onClose={() => setPaletteOpen(false)}
          onOpenSession={openSession}
          onNewSession={(projectId) => startNewSession(projectId)}
          onOpenProjectDialog={() => setProjectDialogOpen(true)}
          onHome={() => navigate('#/')}
        />

        <ProjectDialog
          open={projectDialogOpen}
          submitting={projectSubmitting}
          error={projectError}
          onClose={() => setProjectDialogOpen(false)}
          onSubmit={handleCreateProject}
        />
      </div>
    </DisplayPrefsProvider>
  );
}
