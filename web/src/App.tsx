import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { hasModel } from './lib/api';
import { applyTheme, getStoredTheme, type ThemeId } from './lib/theme';
import { TopBar } from './components/layout/TopBar';
import { PAGE_TABS, tabIdForRoute, type PageId } from './lib/route';
import { Palette } from './components/layout/Palette';
import { ProjectDialog } from './components/project/ProjectDialog';
import { DisplayPrefsProvider, defaultDisplayPrefs } from './lib/display';
import { I18nProvider } from './i18n';
import type { Lang } from './i18n/messages';
import { useSettings } from './hooks/useSettings';
import { useProjects } from './hooks/useProjects';
import { useProjectDialog } from './hooks/useProjectDialog';
import { useTabs } from './hooks/useTabs';
import { useRoute } from './hooks/useRoute';
import { useTabNavigation } from './hooks/useTabNavigation';
import { useSplitView } from './hooks/useSplitView';
import { useShortcuts } from './hooks/useShortcuts';
import { useTurnNotices } from './hooks/useTurnNotices';
import { OfflineNotice } from './components/layout/OfflineNotice';
import { RouteView } from './components/layout/RouteView';
import { PluginsVersionContext } from './hooks/usePluginsVersion';
import { StatusBar } from './components/layout/StatusBar';

export default function App() {
  const { settings, handleSettingsSaved, handleListing } = useSettings();
  const tabState = useTabs();
  const { tabs, openTab } = tabState;
  const { route, navigate } = useRoute(openTab);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [theme, setTheme] = useState<ThemeId>(() => getStoredTheme());

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const displayPrefs = settings?.preferences.display ?? defaultDisplayPrefs;
  // The UI's language: the stored choice, else English (the default).
  const language: Lang = displayPrefs.language ?? 'en';
  const { projects, projectsLoading, projectsError, refreshProjects } = useProjects(language);

  // No polling: the list refreshes when the human looks at it (home, project,
  // palette) and when a turn starts or ends; F5 does the rest.
  const projectsVisible =
    route.name === 'home' ||
    route.name === 'project' ||
    route.name === 'editor' ||
    route.name === 'projects' ||
    route.name === 'chats' ||
    paletteOpen;
  const chatProjectId = projects.find((project) => project.chat)?.id ?? null;

  // The home page reloads its dashboard whenever the sessions change.
  // The first loaded list is what the home page already loaded with: no reload for it.
  const [homeRefreshKey, setHomeRefreshKey] = useState(0);
  const loadedProjects = useRef<typeof projects | null>(null);
  useEffect(() => {
    if (projectsLoading) return;
    if (loadedProjects.current !== null && loadedProjects.current !== projects) {
      setHomeRefreshKey((key) => key + 1);
    }
    loadedProjects.current = projects;
  }, [projects, projectsLoading]);
  useEffect(() => {
    if (projectsVisible) refreshProjects();
  }, [projectsVisible, refreshProjects]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
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

  const projectDialog = useProjectDialog(language, refreshProjects);
  const { openProjectDialog } = projectDialog;
  const { tabInfos, tabInfo, closeTab, pickTab, handleSessionsDeleted, adoptSession } = useTabNavigation(
    tabState,
    route,
    navigate,
    projects,
    language,
    refreshProjects
  );
  const split = useSplitView(route, navigate, tabState, tabInfo, pickTab, adoptSession, refreshProjects);

  const openSession = useCallback(
    (sessionId: string) => {
      openTab(sessionId);
      navigate(`#/s/${encodeURIComponent(sessionId)}`);
    },
    [navigate, openTab]
  );
  const { items: attention, pluginsVersion, online } = useTurnNotices(openSession, displayPrefs.sounds ?? true);

  const startNewSession = useCallback(
    (projectId?: string) => {
      const targetProjectId = projectId ?? projects[0]?.id;
      if (!targetProjectId) {
        openProjectDialog();
        return;
      }
      // No model yet: the setup comes first. A failed check lets the server refuse instead.
      void hasModel()
        .catch(() => true)
        .then((ready) => {
          if (!ready) {
            navigate('#/settings/providers');
            return;
          }
          openTab(`new:${targetProjectId}`);
          navigate(`#/new/${encodeURIComponent(targetProjectId)}`);
        });
    },
    [navigate, projects, openTab, openProjectDialog]
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
      adoptSession(newId, pseudoId);
      navigate(`#/s/${encodeURIComponent(newId)}`);
      refreshProjects();
    },
    [adoptSession, navigate, refreshProjects]
  );

  const shortcutLabels = useShortcuts({
    bindings: settings?.shortcuts,
    route,
    projects,
    tabs,
    chatProjectId,
    navigate,
    pickTab,
    closeTab,
    startNewSession,
    togglePalette: () => setPaletteOpen((open) => !open),
    toggleSplit: split.toggleSplit
  });

  const goHome = useCallback(() => navigate('#/'), [navigate]);
  const openPage = useCallback(
    (page: PageId) => navigate(PAGE_TABS.find((candidate) => candidate.page === page)?.path ?? '#/'),
    [navigate]
  );
  const openProject = useCallback(
    (projectId: string) => navigate(`#/project/${encodeURIComponent(projectId)}`),
    [navigate]
  );

  const newSessionProjects = useMemo(
    () =>
      projects
        .filter((project) => !project.sessionWorktree)
        .sort((a, b) => a.recentRank - b.recentRank)
        .map((project) => ({ id: project.id, name: project.name })),
    [projects]
  );

  return (
    <I18nProvider lang={language}>
    <DisplayPrefsProvider value={displayPrefs}>
      <PluginsVersionContext.Provider value={pluginsVersion}>
      {/* h-full: the html/body/#root chain is 100%, which no page-level
          scaling can push past the window. */}
      <div className="flex h-full flex-col overflow-hidden bg-bg text-text">
        <OfflineNotice visible={!online} />
        <TopBar
          tabs={tabInfos}
          // The highlighted tab follows the route: on home or a project page none is.
          activeTabId={route.name === 'split' ? null : tabIdForRoute(route)}
          attention={attention}
          onOpenAttention={openSession}
          onPickTab={pickTab}
          onCloseTab={closeTab}
          onOpenPalette={() => setPaletteOpen(true)}
          currentProject={currentProject}
          projects={newSessionProjects}
          onNewSession={(projectId) => startNewSession(projectId)}
          onOpenPage={openPage}
          splitActive={route.name === 'split'}
          onToggleSplit={split.toggleSplit}
        />

        <div className="flex min-h-0 grow">
          <RouteView
            route={route}
            projects={projects}
            projectsLoading={projectsLoading}
            projectsError={projectsError}
            settings={settings}
            theme={theme}
            attention={attention}
            homeRefreshKey={homeRefreshKey}
            chatProjectId={chatProjectId}
            currentProjectId={currentProjectId}
            pseudoTabId={route.name === 'new' ? `new:${route.id}` : null}
            newSessionProjects={newSessionProjects}
            split={split}
            onThemeChange={setTheme}
            onSettingsSaved={handleSettingsSaved}
            onListing={handleListing}
            onOpenProjectDialog={openProjectDialog}
            onNewSession={startNewSession}
            onOpenSession={openSession}
            onSessionsDeleted={handleSessionsDeleted}
            onSessionCreated={handleSessionCreated}
            onRefreshProjects={refreshProjects}
            onHome={goHome}
            onOpenPage={openPage}
            onOpenProject={openProject}
          />
        </div>

        <StatusBar
          sessionId={statusBarIds.sessionId ?? null}
          projectId={statusBarIds.projectId ?? null}
        />

        <Palette
          open={paletteOpen}
          projects={projects}
          shortcuts={shortcutLabels}
          onClose={() => setPaletteOpen(false)}
          onOpenSession={openSession}
          onNewSession={startNewSession}
          onOpenProjectDialog={openProjectDialog}
          onHome={goHome}
          onOpenPage={openPage}
          onOpenProject={openProject}
        />

        <ProjectDialog {...projectDialog.dialogProps} />
      </div>
      </PluginsVersionContext.Provider>
    </DisplayPrefsProvider>
    </I18nProvider>
  );
}
