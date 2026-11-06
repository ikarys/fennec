import { useCallback, useEffect, useMemo, useRef, useState, lazy, Suspense } from 'react';
import type { Project, ProviderListing, SettingsResponse } from './lib/api';
import { createProject, getProjects, getSettings, setProjectFavorite } from './lib/api';
import { applyTheme, getStoredTheme, type ThemeId } from './lib/theme';
import { TopBar, type TabInfo } from './components/layout/TopBar';
import {
  ACTIVE_TAB_KEY,
  PAGE_TABS,
  TABS_KEY,
  loadActiveTab,
  loadTabs,
  parseRoute,
  splitPath,
  tabIdForRoute,
  tabPath,
  type Route,
} from './lib/route';
import { HomePage } from './pages/HomePage';
import { SessionPage } from './pages/SessionPage';
import { SplitPage } from './pages/SplitPage';
import { SettingsPage } from './pages/SettingsPage';
import { InsightPage } from './pages/InsightPage';
import { Palette } from './components/Palette';
import { ProjectDialog } from './components/project/ProjectDialog';
import { DisplayPrefsProvider, defaultDisplayPrefs } from './lib/display';
import { I18nProvider } from './i18n';
import { t } from './i18n/messages';
import type { Lang } from './i18n/messages';
import { useTurnNotices } from './hooks/useTurnNotices';
import { PluginsVersionContext } from './hooks/usePluginsVersion';
import { StatusBar } from './components/layout/StatusBar';

const WorkflowsPage = lazy(() => import('./pages/WorkflowsPage'));
const ProjectPage = lazy(() => import('./pages/ProjectPage'));
const BacklogPage = lazy(() => import('./pages/BacklogPage'));
const ChangesPage = lazy(() => import('./pages/ChangesPage'));

export default function App() {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash));
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
    const onHashChange = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, []);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

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

  // A page route (settings, insight, workflows, backlog) gets its tab in the
  // bar the moment it is on screen, so the active page is never unlisted.
  const pageTabIdForRoute =
    route.name === 'settings' ||
    route.name === 'insight' ||
    route.name === 'backlog' ||
    route.name === 'workflows'
      ? `page:${route.name}`
      : null;
  useEffect(() => {
    if (!pageTabIdForRoute) return;
    setTabs((previous) => (previous.includes(pageTabIdForRoute) ? previous : [...previous, pageTabIdForRoute]));
    setActiveTabId(pageTabIdForRoute);
  }, [pageTabIdForRoute]);

  // `projectsLoading` is only the first load: a refresh keeps the list on
  // screen instead of flashing a loading state.
  const displayPrefs = settings?.preferences.display ?? defaultDisplayPrefs;
  // The UI's language: the stored choice, else English (the default).
  const language: Lang = displayPrefs.language ?? 'en';
  const refreshProjects = useCallback(async () => {
    try {
      const data = await getProjects();
      setProjects(data);
      setProjectsError(null);
    } catch (error) {
      setProjectsError(error instanceof Error ? error.message : t(language, 'projectsLoadError'));
    } finally {
      setProjectsLoading(false);
    }
  }, [language]);

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
      setRoute(parseRoute(window.location.hash));
    }
  }, []);

  const tabInfos = useMemo<TabInfo[]>(() => {
    const infos: TabInfo[] = tabs.map((id) => {
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
    });
    return infos;
  }, [tabs, projects, language]);

  const openSession = useCallback(
    (sessionId: string) => {
      setTabs((previous) => (previous.includes(sessionId) ? previous : [...previous, sessionId]));
      setActiveTabId(sessionId);
      navigate(`#/s/${encodeURIComponent(sessionId)}`);
    },
    [navigate]
  );
  const { items: attention, pluginsVersion } = useTurnNotices(openSession);

  const closeTab = useCallback(
    (tabId: string) => {
      const next = tabs.filter((id) => id !== tabId);
      setTabs(next);
      // Only the tab that is on screen (per the route) triggers a navigation.
      // activeTabId can be stale when a session was opened by URL/bookmark, so
      // it is not a reliable "is this tab showing" signal.
      if (tabIdForRoute(route) !== tabId) return;
      // A page tab closing while active returns home; a session tab falls back
      // to the last remaining session tab, or home when none are left. Page
      // tabs are re-derived from the route, so they are not a close target.
      if (tabId.startsWith('page:')) {
        setActiveTabId(null);
        navigate('#/');
        return;
      }
      const fallback = next.filter((id) => !id.startsWith('page:')).pop() ?? null;
      setActiveTabId(fallback);
      navigate(fallback ? tabPath(fallback) : '#/');
    },
    [tabs, route, navigate]
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

  // The `+` of the bar: the project of the session on screen, else the most recent one.
  const currentProjectId = useMemo(() => {
    const sessionId = route.name === 'session' || route.name === 'changes' ? route.id : route.name === 'new' ? route.id : null;
    if (!sessionId) return undefined;
    if (route.name === 'new') return sessionId;
    return projects.find((project) => project.sessions.some((session) => session.id === sessionId))?.id;
  }, [route, projects]);

  const currentProject = useMemo(() => {
    const project = projects.find((candidate) => candidate.id === currentProjectId);
    return project ? { id: project.id, name: project.name } : null;
  }, [projects, currentProjectId]);

  // The status bar's surfaces: the session on screen and its project, a
  // project page route its own id; the global blocks come back regardless.
  const statusBarIds = useMemo(() => {
    if (route.name === 'session' || route.name === 'changes') return { sessionId: route.id, projectId: currentProjectId ?? undefined };
    if (route.name === 'new') return { sessionId: undefined, projectId: route.id };
    if (route.name === 'project') return { sessionId: undefined, projectId: route.projectId };
    return { sessionId: undefined, projectId: undefined };
  }, [route, currentProjectId]);

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
        setProjectError(error instanceof Error ? error.message : t(language, 'projectOpenError'));
      } finally {
        setProjectSubmitting(false);
      }
    },
    [refreshProjects, language]
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

  // Re-clicking the tab of the page already on screen keeps its sub-tab
  // (settings tab, project id, …) instead of resetting to the page's root.
  const basePathOf = (hash: string): string => {
    const segments = hash.slice(1).split('/').filter((segment) => segment.length > 0);
    return `#/${segments.slice(0, 2).join('/')}`;
  };
  const pickTab = useCallback(
    (id: string) => {
      setActiveTabId(id);
      const target = tabPath(id);
      if (!id.startsWith('page:') || basePathOf(window.location.hash) !== target) {
        navigate(target);
      }
    },
    [navigate]
  );

  const pseudoTabId = route.name === 'new' ? `new:${route.id}` : null;

  return (
    <I18nProvider lang={language}>
    <DisplayPrefsProvider value={displayPrefs}>
      <PluginsVersionContext.Provider value={pluginsVersion}>
      {/* h-full: the html/body/#root chain is 100%, which no page-level
          scaling can push past the window. */}
      <div className="flex h-full flex-col overflow-hidden bg-bg text-text">
        <TopBar
          tabs={tabInfos}
          activeTabId={activeTabId}
          attention={attention}
          onOpenAttention={openSession}
          onPickTab={pickTab}
          onCloseTab={closeTab}
          onOpenPalette={() => setPaletteOpen(true)}
          currentProject={currentProject}
          projects={projects.filter((project) => !project.sessionWorktree).map((project) => ({ id: project.id, name: project.name }))}
          onNewSession={(projectId) => startNewSession(projectId)}
          onOpenPage={(page) => navigate(PAGE_TABS.find((candidate) => candidate.page === page)?.path ?? '#/')}
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
              onToggleFavorite={(projectId, favorite) => void setProjectFavorite(projectId, favorite).then(refreshProjects)}
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
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">{t(language, 'loading')}</div>}>
              <WorkflowsPage
                projects={projects}
                projectId={route.projectId}
                workflowId={route.workflowId}
                isNew={route.isNew}
              />
            </Suspense>
          ) : route.name === 'project' ? (
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">{t(language, 'loading')}</div>}>
              <ProjectPage projects={projects} projectId={route.projectId} tab={route.tab} sub={route.sub} onOpenSession={openSession} onNewSession={(projectId) => startNewSession(projectId)} onDeleted={refreshProjects} onSessionsDeleted={handleSessionsDeleted} />
            </Suspense>
          ) : route.name === 'insight' ? (
            <InsightPage />
          ) : route.name === 'backlog' ? (
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">{t(language, 'loading')}</div>}>
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
              onOpenPage={(page) => navigate(PAGE_TABS.find((candidate) => candidate.page === page)?.path ?? '#/')}
              onOpenProject={(projectId) => navigate(`#/project/${encodeURIComponent(projectId)}`)}
            />
          ) : route.name === 'changes' ? (
            <Suspense fallback={<div className="flex grow items-center justify-center text-muted">{t(language, 'loading')}</div>}>
              <ChangesPage sessionId={route.id} />
            </Suspense>
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

        <StatusBar
          sessionId={statusBarIds.sessionId ?? null}
          projectId={statusBarIds.projectId ?? null}
        />

        <Palette
          open={paletteOpen}
          projects={projects}
          onClose={() => setPaletteOpen(false)}
          onOpenSession={openSession}
          onNewSession={(projectId) => startNewSession(projectId)}
          onOpenProjectDialog={() => setProjectDialogOpen(true)}
          onHome={() => navigate('#/')}
          onOpenPage={(page) => navigate(PAGE_TABS.find((candidate) => candidate.page === page)?.path ?? '#/')}
          onOpenProject={(projectId) => navigate(`#/project/${encodeURIComponent(projectId)}`)}
        />

        <ProjectDialog
          open={projectDialogOpen}
          submitting={projectSubmitting}
          error={projectError}
          onClose={() => setProjectDialogOpen(false)}
          onSubmit={handleCreateProject}
        />
      </div>
      </PluginsVersionContext.Provider>
    </DisplayPrefsProvider>
    </I18nProvider>
  );
}
