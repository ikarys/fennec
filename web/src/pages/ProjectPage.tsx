/** Project page: section rail and content for one project. */
import { useEffect, useState, type ReactNode } from 'react';
import { Tabs } from '../components/ui/Tabs';
import { Button, buttonClass } from '../components/ui/Button';
import {
  ActivityIcon,
  EditIcon,
  InsightIcon,
  LayersIcon,
  PlusIcon,
  ProjectIcon,
  TrashIcon,
} from '../components/ui/Icons';
import { editorPath, projectTab, withTask, type ProjectTab } from '../lib/route';
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
import { BudgetTile } from '../components/project/BudgetTile';
import { DiskSection } from '../components/project/DiskSection';
import { IndicatorsSection } from '../components/project/IndicatorsSection';
import { CanvasSection } from '../components/project/CanvasSection';
import {
  createSession,
  getProjectOverview,
  type Project,
  type ProjectOverview,
} from '../lib/api';
import { type IdeaView } from '../lib/ideas';
import { useT } from '../i18n';


const TABS = [
  { id: 'work', label: 'projectTabWork', Icon: ActivityIcon },
  { id: 'understand', label: 'projectTabUnderstand', Icon: LayersIcon },
  { id: 'health', label: 'projectTabHealth', Icon: InsightIcon },
] as const;

/**
 * The map section fetches the project overview on its own so the sections
 * around it do not wait for it.
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

const anchorId = (name: string) => `project-${name}`;

export default function ProjectPage({
  projects,
  projectId,
  tab,
  sub,
  task,
  onOpenSession,
  onNewSession,
  onDeleted,
  onSessionsDeleted,
}: {
  projects: Project[];
  projectId: string;
  tab: string | null;
  sub: string | null;
  task: string | null;
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

  const { tab: activeTab, anchor } = projectTab(tab);

  useEffect(() => {
    if (anchor) {
      document.getElementById(anchorId(anchor))?.scrollIntoView({ block: 'start' });
    }
  }, [activeTab, anchor]);

  const handleTabSelect = (id: ProjectTab) => {
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
            className={buttonClass({ variant: 'outline', size: 'sm', iconOnly: true })}
          >
            <EditIcon className="h-4 w-4" />
          </a>
          <ProjectOpenActions projectId={projectId} editorUrl={project.editorUrl} />
          <Button
            variant="danger"
            size="sm"
            iconOnly
            title={t('deleteProjectBtn')}
            aria-label={t('deleteProjectBtn')}
            onClick={() => setDeleting(true)}
          >
            <TrashIcon className="h-3.5 w-3.5" />
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
          {activeTab === 'work' ? (
            <div className="flex flex-col gap-6">
              <DashboardSection projectId={projectId} />
              <section id={anchorId('tasks')} className="scroll-mt-4">
                <TasksSection
                  projectId={projectId}
                  sub={sub}
                  task={task}
                  onOpenTask={(ideaId) => {
                    window.location.hash = withTask(window.location.hash, ideaId);
                  }}
                  onFrame={handleFrame}
                  onOpenSession={onOpenSession}
                  onSelectSub={(s) => {
                    window.location.hash = `#/project/${encodeURIComponent(projectId)}/tasks/${encodeURIComponent(s)}`;
                  }}
                />
              </section>
              <section id={anchorId('sessions')} className="scroll-mt-4">
                <SessionsSection projectName={project.name} sessions={project.sessions} onOpenSession={onOpenSession} onSessionsDeleted={onSessionsDeleted} />
              </section>
            </div>
          ) : null}
          {activeTab === 'understand' ? (
            <div className="flex flex-col gap-6">
              <section id={anchorId('map')} className="scroll-mt-4">
                <MapTab projectId={projectId} />
              </section>
              <section id={anchorId('risks')} className="scroll-mt-4">
                <RiskSection projectId={projectId} busy={analyzing} onStart={(prompt, mode) => void startSession(prompt, mode)} />
              </section>
              <section id={anchorId('decisions')} className="scroll-mt-4">
                <DecisionsSection projectId={projectId} />
              </section>
              <section id={anchorId('analyses')} className="scroll-mt-4">
                <AnalysesSection
                  busy={analyzing}
                  error={analyzeError}
                  sessions={project.sessions}
                  onStart={(prompt, mode) => void startSession(prompt, mode)}
                  onOpenSession={onOpenSession}
                />
              </section>
              <section id={anchorId('canvas')} className="scroll-mt-4">
                <CanvasSection projectId={projectId} />
              </section>
            </div>
          ) : null}
          {activeTab === 'health' ? (
            <div className="flex flex-col gap-6">
              <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
                <BudgetTile projectId={projectId} />
              </div>
              <IndicatorsSection projectId={projectId} />
              <DiskSection projectId={projectId} onOpenSession={onOpenSession} />
            </div>
          ) : null}
        </div>
      </div>
    </>,
  );
}
