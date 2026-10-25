import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Config, Project, SettingsResponse } from './lib/api';
import { createProject, getConfig, getProjects, getSettings } from './lib/api';
import { applyTheme, getStoredTheme, type ThemeId } from './lib/theme';
import { TopBar, type TabInfo } from './components/layout/TopBar';
import { HomePage } from './pages/HomePage';
import { SessionPage } from './pages/SessionPage';
import { SettingsPage } from './pages/SettingsPage';
import { Palette } from './components/Palette';
import { ProjectDialog } from './components/project/ProjectDialog';
import { DisplayPrefsProvider, defaultDisplayPrefs } from './lib/display';

type Route =
  | { name: 'home' }
  | { name: 'session'; id: string }
  | { name: 'new'; id: string }
  | { name: 'settings' };

const TABS_KEY = 'fennec.tabs';
const ACTIVE_TAB_KEY = 'fennec.activeTab';

function parseRoute(): Route {
  const hash = window.location.hash;
  if (hash === '#/settings') {
    return { name: 'settings' };
  }
  if (hash.startsWith('#/s/')) {
    const id = decodeURIComponent(hash.slice(4));
    if (id.startsWith('new:')) {
      return { name: 'new', id: id.slice(4) };
    }
    return { name: 'session', id };
  }
  if (hash.startsWith('#/new/')) {
    return { name: 'new', id: decodeURIComponent(hash.slice(6)) };
  }
  return { name: 'home' };
}

/** The route of a tab: a pseudo `new:` tab opens the new-session page. */
function tabPath(tabId: string): string {
  return tabId.startsWith('new:')
    ? `#/new/${encodeURIComponent(tabId.slice(4))}`
    : `#/s/${encodeURIComponent(tabId)}`;
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
  const [config, setConfig] = useState<Config | null>(null);
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [tabs, setTabs] = useState<string[]>(() => loadTabs());
  const [activeTabId, setActiveTabId] = useState<string | null>(() => loadActiveTab());
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [projectDialogOpen, setProjectDialogOpen] = useState(false);
  const [projectSubmitting, setProjectSubmitting] = useState(false);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemeId>(() => getStoredTheme());

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
    getConfig()
      .then(setConfig)
      .catch(() => setConfig(null));
  }, []);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch(() => setSettings(null));
  }, []);

  const handleSettingsSaved = useCallback((next: SettingsResponse) => {
    setSettings(next);
    // The resolved model and window shown in the context panel may change.
    getConfig()
      .then(setConfig)
      .catch(() => setConfig(null));
  }, []);

  // No polling: the list refreshes when the human looks at it (home, palette)
  // and when a turn starts or ends; F5 does the rest.
  const homeVisible = route.name === 'home' || paletteOpen;
  useEffect(() => {
    if (homeVisible) refreshProjects();
  }, [homeVisible, refreshProjects]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
      if (event.key === 'Escape') {
        setPaletteOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const displayPrefs = settings?.preferences.display ?? defaultDisplayPrefs;

  // The UI is laid out on the default size (the CSS body size); the chosen
  // size zooms the whole page around it.
  useEffect(() => {
    document.documentElement.style.zoom = String(displayPrefs.fontSize / defaultDisplayPrefs.fontSize);
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

  const pseudoTabId = route.name === 'new' ? `new:${route.id}` : null;

  return (
    <DisplayPrefsProvider value={displayPrefs}>
      <div className="flex h-dvh flex-col overflow-hidden bg-bg text-text">
        <TopBar
          tabs={tabInfos}
          activeTabId={activeTabId}
          theme={theme}
          onPickTab={(id) => {
            setActiveTabId(id);
            navigate(tabPath(id));
          }}
          onCloseTab={closeTab}
          onNewSession={() => startNewSession()}
          onOpenPalette={() => setPaletteOpen(true)}
          onThemeChange={setTheme}
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
            />
          ) : route.name === 'settings' ? (
            <SettingsPage settings={settings} onSaved={handleSettingsSaved} />
          ) : (
            <SessionPage
              sessionId={route.name === 'session' ? route.id : null}
              projectId={route.name === 'new' ? route.id : null}
              pseudoTabId={pseudoTabId}
              projects={projects}
              config={config}
              onSessionCreated={handleSessionCreated}
              onRunningChanged={refreshProjects}
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
