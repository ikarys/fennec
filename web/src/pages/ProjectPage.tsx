/** Project page: section rail and content for one project. */
import { useEffect, useState, type ReactNode } from 'react';
import { Tabs } from '../components/ui/Tabs';
import { Button, buttonClass } from '../components/ui/Button';
import {
  ActivityIcon,
  AlertIcon,
  BulbIcon,
  EditIcon,
  DecisionIcon,
  InsightIcon,
  LayersIcon,
  PlusIcon,
  ProjectIcon,
  SearchIcon,
  TerminalIcon,
  TrashIcon,
  WandIcon,
} from '../components/ui/Icons';
import { PageHeader } from '../components/ui/PageHeader';
import { DeleteProjectDialog } from '../components/project/DeleteProjectDialog';
import { ProjectOpenActions } from '../components/project/ProjectOpenActions';
import { TasksSection } from '../components/project/ideas/TasksSection';
import { MapSection } from '../components/project/MapSection';
import { DecisionsSection } from '../components/project/DecisionsSection';
import { RiskSection } from '../components/project/RiskSection';
import { AnalysesSection } from '../components/project/AnalysesSection';
import { SessionsSection } from '../components/project/SessionsSection';
import { DashboardSection } from '../components/project/DashboardSection';
import { IndicatorsSection } from '../components/project/IndicatorsSection';
import { CanvasSection } from '../components/project/CanvasSection';
import {
  createSession,
  getProjectOverview,
  type Project,
  type ProjectOverview,
} from '../lib/api';
import { type IdeaView } from '../lib/ideas';
import { editorPath } from '../lib/route';
import { useT } from '../i18n';


const TABS = [
  { id: 'overview', label: 'tasksTabOverview', Icon: ActivityIcon },
  { id: 'indicators', label: 'indicatorsTab', Icon: InsightIcon },
  { id: 'tasks', label: 'tasks', Icon: BulbIcon },
  { id: 'sessions', label: 'sessionsSectionTitle', Icon: TerminalIcon },
  { id: 'map', label: 'moduleMapTitle', Icon: LayersIcon },
  { id: 'risks', label: 'risksLabel', Icon: AlertIcon },
  { id: 'decisions', label: 'projectPageDecisions', Icon: DecisionIcon },
  { id: 'canvas', label: 'canvasLibraryTab', Icon: WandIcon },
  { id: 'analyses', label: 'analysesTitle', Icon: SearchIcon },
] as const;

type TabId = (typeof TABS)[number]['id'];

/**
 * The map tab fetches the project overview on its own so the tasks and doc
 * tabs do not wait for it.
 */
function MapTab({ projectId }: { projectId: string }) {
  const t = useT();
  const [data, setData] = useState<ProjectOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setData(null);
    getProjectOverview(projectId)
      .then((overview) => {
        if (!cancelled) {
          setData(overview);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : t('unknownError'));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  if (loading) return <p className="text-muted">{t('loading')}</p>;
  if (error) return <p className="text-err">{error}</p>;
  if (!data) return <p className="text-muted">{t('loading')}</p>;

  return (
    <MapSection
      projectId={projectId}
      map={data.map}
      mapFile={data.mapFile}
      drift={data.drift}
      onWritten={(state) =>
        setData((prev) => (prev ? { ...prev, mapFile: state } : prev))
      }
    />
  );
}

export default function ProjectPage({
  projects,
  projectId,
  tab,
  sub,
  onOpenSession,
  onNewSession,
  onDeleted,
  onSessionsDeleted,
}: {
  projects: Project[];
  projectId: string;
  tab: string | null;
  sub: string | null;
  onOpenSession: (sessionId: string) => void;
  onNewSession: (projectId: string) => void;
  /** Home and project pages both show the list, so going home does not refresh it. */
  onDeleted: () => void;
  onSessionsDeleted: (ids: string[]) => void;
}) {
  const t = useT();
  const project = projects.find((p) => p.id === projectId) ?? null;
  const [analyzing, setAnalyzing] = useState(false);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Unknown or null tab defaults to 'overview'.
  const activeTab: TabId = TABS.find((t) => t.id === tab)?.id ?? 'overview';

  const handleTabSelect = (id: TabId) => {
    window.location.hash = `#/project/${encodeURIComponent(projectId)}/${id}`;
  };

  const startSession = async (prompt: string, mode: string) => {
    setAnalyzing(true);
    setAnalyzeError(null);
    try {
      const created = await createSession(projectId, prompt, mode);
      onOpenSession(created.id);
    } catch (err) {
      setAnalyzeError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setAnalyzing(false);
    }
  };

  // Opens a new plan session with the idea in the composer: the human edits it, then sends.
  const handleFrame = async (idea: IdeaView) => {
    window.location.hash = `#/new/${encodeURIComponent(projectId)}?frame=${encodeURIComponent(idea.id)}`;
  };

  const shell = (children: ReactNode) => (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto max-w-[1100px]">{children}</div>
    </main>
  );

  if (!project) {
    return shell(<p className="text-muted">{t('projectPageNotFound')}</p>);
  }

  const header = (
    <PageHeader
      Icon={ProjectIcon}
      title={project.name}
      subtitle={
        <span className="break-all font-mono text-xs">
          {project.workdir} · ⎇ {project.branch ?? t('noBranch')}
          {project.remote ? (
            <>
              {' · '}
              <a
                href={project.remote}
                target="_blank"
                rel="noreferrer"
                className="text-accent-strong underline"
                title={project.remote}
              >
                {project.remote.replace(/^https?:\/\//, '')}
              </a>
            </>
          ) : null}
        </span>
      }
      actions={
        <>
          <a
            href={editorPath(projectId, null)}
            title={t('editorOpen')}
            aria-label={t('editorOpen')}
            className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
          >
            <EditIcon className="h-4 w-4" />
          </a>
          <ProjectOpenActions projectId={projectId} editorUrl={project.editorUrl} />
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('deleteProjectBtn')}
            aria-label={t('deleteProjectBtn')}
            onClick={() => setDeleting(true)}
          >
            <TrashIcon className="h-4 w-4" />
          </Button>
          <Button variant="primary" size="md" onClick={() => onNewSession(projectId)} title={t('newSessionInProject')}>
            <PlusIcon className="h-4 w-4" />
            {t('newSession')}
          </Button>
        </>
      }
    />
  );

  return shell(
    <>
      {header}
      {deleting && (
        <DeleteProjectDialog
          project={project}
          onClose={() => setDeleting(false)}
          onDeleted={() => {
            setDeleting(false);
            onDeleted();
            window.location.hash = '#/';
          }}
        />
      )}
      <div className="mt-6 flex flex-col gap-6 md:flex-row">
        <Tabs
          label={t('projectPageSectionsLabel')}
          tabs={TABS.map((item) => ({ ...item, label: t(item.label) }))}
          active={activeTab}
          onSelect={handleTabSelect}
        />
        <div className="min-w-0 grow">
          {activeTab === 'overview' ? (
            <DashboardSection projectId={projectId} onOpenSession={onOpenSession} />
          ) : null}
          {activeTab === 'indicators' ? <IndicatorsSection projectId={projectId} /> : null}
          {activeTab === 'tasks' ? (
            <TasksSection
              projectId={projectId}
              sub={sub}
              onFrame={handleFrame}
              onOpenSession={onOpenSession}
              onSelectSub={(s) => {
                window.location.hash = `#/project/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(s)}`;
              }}
            />
          ) : null}
          {activeTab === 'sessions' ? <SessionsSection projectName={project.name} sessions={project.sessions} onOpenSession={onOpenSession} onSessionsDeleted={onSessionsDeleted} /> : null}
          {activeTab === 'map' ? <MapTab projectId={projectId} /> : null}
          {activeTab === 'risks' ? <RiskSection projectId={projectId} busy={analyzing} onStart={(prompt, mode) => void startSession(prompt, mode)} /> : null}
          {activeTab === 'decisions' ? <DecisionsSection projectId={projectId} /> : null}
          {activeTab === 'canvas' ? <CanvasSection projectId={projectId} /> : null}
          {activeTab === 'analyses' ? (
            <AnalysesSection
              busy={analyzing}
              error={analyzeError}
              sessions={project.sessions}
              onStart={(prompt, mode) => void startSession(prompt, mode)}
              onOpenSession={onOpenSession}
            />
          ) : null}
        </div>
      </div>
    </>,
  );
}
