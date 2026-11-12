// The page the current route shows: the route-to-page switch, with the heavy pages loaded on demand.
import { lazy, Suspense, type ReactNode } from 'react';
import type { Project, ProviderListing, SettingsResponse } from '../../lib/api';
import type { Route } from '../../lib/route';
import type { ThemeId } from '../../lib/theme';
import type { PageId } from '../../lib/route';
import type { useSplitView } from '../../hooks/useSplitView';
import type { useTurnNotices } from '../../hooks/useTurnNotices';
import { useT } from '../../i18n';
import { HomePage } from '../../pages/HomePage';
import { ProjectsPage } from '../../pages/ProjectsPage';
import { ChatsPage } from '../../pages/ChatsPage';
import { SessionPage } from '../../pages/SessionPage';
import { SplitPage } from '../../pages/SplitPage';
import { SettingsPage } from '../../pages/SettingsPage';
import { InsightPage } from '../../pages/InsightPage';
import { setProjectFavorite } from '../../lib/api';

const WorkflowsPage = lazy(() => import('../../pages/WorkflowsPage'));
const ProjectPage = lazy(() => import('../../pages/ProjectPage'));
const BacklogPage = lazy(() => import('../../pages/BacklogPage'));
const ChangesPage = lazy(() => import('../../pages/ChangesPage'));
const EditorPage = lazy(() => import('../../pages/EditorPage'));

/** A lazily loaded page, with the loading line shown while its code arrives. */
function Lazy({ children }: { children: ReactNode }) {
  const t = useT();
  return (
    <Suspense fallback={<div className="flex grow items-center justify-center text-muted">{t('loading')}</div>}>
      {children}
    </Suspense>
  );
}

interface RouteViewProps {
  route: Route;
  projects: Project[];
  projectsLoading: boolean;
  projectsError: string | null;
  settings: SettingsResponse | null;
  theme: ThemeId;
  attention: ReturnType<typeof useTurnNotices>['items'];
  homeRefreshKey: number;
  chatProjectId: string | null;
  currentProjectId: string | undefined;
  pseudoTabId: string | null;
  newSessionProjects: { id: string; name: string }[];
  split: ReturnType<typeof useSplitView>;
  onThemeChange: (theme: ThemeId) => void;
  onSettingsSaved: (next: SettingsResponse) => void;
  onListing: (listing: ProviderListing) => void;
  onOpenProjectDialog: () => void;
  onNewSession: (projectId?: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
  onSessionCreated: (newId: string, pseudoId: string | null) => void;
  onRefreshProjects: () => void;
  onHome: () => void;
  onOpenPage: (page: PageId) => void;
  onOpenProject: (projectId: string) => void;
}

export function RouteView(props: RouteViewProps) {
  const { route, projects, attention, onNewSession, onOpenSession, onOpenProjectDialog, onSessionsDeleted, onRefreshProjects } = props;
  switch (route.name) {
    case 'home':
      return (
        <HomePage
          projectCount={projects.filter((project) => !project.chat).length}
          attention={attention}
          refreshKey={props.homeRefreshKey}
          onOpenProjectDialog={onOpenProjectDialog}
          onNewChat={props.chatProjectId ? () => onNewSession(props.chatProjectId ?? undefined) : null}
          onNewSession={onNewSession}
          onOpenSession={onOpenSession}
        />
      );
    case 'projects':
      return (
        <ProjectsPage
          projects={projects}
          loading={props.projectsLoading}
          error={props.projectsError}
          onOpenProjectDialog={onOpenProjectDialog}
          onNewSession={onNewSession}
          onOpenSession={onOpenSession}
          onSessionsDeleted={onSessionsDeleted}
          onToggleFavorite={(projectId, favorite) => void setProjectFavorite(projectId, favorite).then(onRefreshProjects)}
        />
      );
    case 'chats':
      return (
        <ChatsPage
          projects={projects}
          loading={props.projectsLoading}
          error={props.projectsError}
          onNewSession={onNewSession}
          onOpenSession={onOpenSession}
          onSessionsDeleted={onSessionsDeleted}
        />
      );
    case 'settings':
      return (
        <SettingsPage
          page={route.tab}
          settings={props.settings}
          theme={props.theme}
          onThemeChange={props.onThemeChange}
          onSaved={props.onSettingsSaved}
          onListing={props.onListing}
        />
      );
    case 'workflows':
      return (
        <Lazy>
          <WorkflowsPage projects={projects.filter((project) => !project.chat)} projectId={route.projectId} workflowId={route.workflowId} isNew={route.isNew} />
        </Lazy>
      );
    case 'project':
      return (
        <Lazy>
          <ProjectPage
            projects={projects}
            projectId={route.projectId}
            tab={route.tab}
            sub={route.sub}
            task={route.task}
            onOpenSession={onOpenSession}
            onNewSession={onNewSession}
            onDeleted={onRefreshProjects}
            onSessionsDeleted={onSessionsDeleted}
          />
        </Lazy>
      );
    case 'editor':
      return (
        <Lazy>
          <EditorPage key={route.projectId} projects={projects} projectId={route.projectId} path={route.path} sessionId={route.session} />
        </Lazy>
      );
    case 'insight':
      return <InsightPage />;
    case 'backlog':
      return (
        <Lazy>
          <BacklogPage />
        </Lazy>
      );
    case 'split':
      return (
        <SplitPage
          left={route.left}
          right={route.right}
          onClosePane={props.split.closePane}
          paneTabs={props.split.paneTabInfos}
          newSessionProjects={props.newSessionProjects}
          onPickTabInPane={props.split.pickTabInPane}
          onCloseTabInPane={props.split.closeTabInPane}
          projects={projects}
          onChooseSession={props.split.openSessionInPane}
          onNewSessionInPane={props.split.newSessionInPane}
          onSessionCreated={props.split.sessionCreatedInPane}
          onRunningChanged={onRefreshProjects}
          onRenamed={onRefreshProjects}
          onOpenProjectDialog={onOpenProjectDialog}
          onHome={props.onHome}
          onOpenPage={props.onOpenPage}
          onOpenProject={props.onOpenProject}
        />
      );
    case 'changes':
      return (
        <Lazy>
          <ChangesPage sessionId={route.id} initialPath={route.path} projectId={props.currentProjectId ?? null} />
        </Lazy>
      );
    default:
      return (
        <SessionPage
          sessionId={route.name === 'session' ? route.id : null}
          projectId={route.name === 'new' ? route.id : null}
          launchWorkflow={route.name === 'new' ? route.launchWorkflow : null}
          frameIdeaId={route.name === 'new' ? route.frameIdeaId : null}
          fromSessionId={route.name === 'new' ? route.fromSessionId : null}
          pseudoTabId={props.pseudoTabId}
          projects={projects}
          onSessionCreated={props.onSessionCreated}
          onRunningChanged={onRefreshProjects}
          onRenamed={onRefreshProjects}
        />
      );
  }
}
