import { useEffect, useState, type ReactNode } from 'react';
import { Tabs } from '../components/ui/Tabs';
import { buttonClass } from '../components/ui/Button';
import { EditIcon } from '../components/Icons';
import { TasksSection } from '../components/project/ideas/TasksSection';
import { MapSection } from '../components/project/MapSection';
import { DocSection } from '../components/project/DocSection';
import { RiskSection } from '../components/project/RiskSection';
import { ProjectRulesSummary } from '../components/rules/RulesSection';
import { AnalysesSection } from '../components/project/AnalysesSection';
import { DiskSection } from '../components/project/DiskSection';
import { DashboardSection } from '../components/project/DashboardSection';
import {
  createSession,
  getProjectOverview,
  type Project,
  type ProjectOverview,
} from '../lib/api';
import { type IdeaView } from '../lib/ideas';

const TABS = [
  { id: 'overview', label: "Vue d'ensemble" },
  { id: 'tasks', label: 'Tâches' },
  { id: 'map', label: 'Carte des modules' },
  { id: 'risks', label: 'Risques' },
  { id: 'rules', label: 'Règles' },
  { id: 'doc', label: 'Doc' },
  { id: 'analyses', label: 'Analyses' },
  { id: 'disk', label: 'Espace disque' },
] as const;

type TabId = (typeof TABS)[number]['id'];

/**
 * The map tab fetches the project overview on its own so the tasks and doc
 * tabs do not wait for it.
 */
function MapTab({ projectId }: { projectId: string }) {
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
          setError(err instanceof Error ? err.message : 'Erreur inconnue');
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  if (loading) return <p className="text-muted">Chargement…</p>;
  if (error) return <p className="text-err">{error}</p>;
  if (!data) return <p className="text-muted">Chargement…</p>;

  return (
    <MapSection
      projectId={projectId}
      map={data.map}
      mapFile={data.mapFile}
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
}: {
  projects: Project[];
  projectId: string;
  tab: string | null;
  sub: string | null;
  onOpenSession: (sessionId: string) => void;
}) {
  const project = projects.find((p) => p.id === projectId) ?? null;
  const [analyzing, setAnalyzing] = useState(false);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);

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
      setAnalyzeError(err instanceof Error ? err.message : 'Erreur inconnue');
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
    return shell(<p className="text-muted">Projet introuvable.</p>);
  }

  const header = (
    <>
      <div className="flex items-center gap-3">
        <h1 className="text-2xl font-semibold">
          {project.name}
        </h1>
        <a
          href={project.editorUrl}
          title="Ouvrir dans VS Code"
          aria-label="Ouvrir dans VS Code"
          className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
        >
          <EditIcon className="h-4 w-4" />
        </a>
      </div>
      <p className="mt-0.5 break-all font-mono text-xs text-muted">
        {project.workdir} · ⎇ {project.branch ?? 'sans branche'}
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
      </p>
    </>
  );

  return shell(
    <>
      {header}
      <div className="mt-6 flex flex-col gap-6 md:flex-row">
        <Tabs
          label="Sections du projet"
          tabs={[...TABS]}
          active={activeTab}
          onSelect={handleTabSelect}
        />
        <div className="min-w-0 grow">
          {activeTab === 'overview' ? (
            <DashboardSection projectId={projectId} onOpenSession={onOpenSession} />
          ) : null}
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
          {activeTab === 'map' ? <MapTab projectId={projectId} /> : null}
          {activeTab === 'risks' ? <RiskSection projectId={projectId} /> : null}
          {activeTab === 'rules' ? <ProjectRulesSummary projectId={projectId} /> : null}
          {activeTab === 'doc' ? <DocSection projectId={projectId} /> : null}
          {activeTab === 'analyses' ? (
            <AnalysesSection
              busy={analyzing}
              error={analyzeError}
              sessions={project.sessions}
              onStart={(prompt, mode) => void startSession(prompt, mode)}
              onOpenSession={onOpenSession}
            />
          ) : null}
          {activeTab === 'disk' ? <DiskSection projectId={projectId} onOpenSession={onOpenSession} /> : null}
        </div>
      </div>
    </>,
  );
}
