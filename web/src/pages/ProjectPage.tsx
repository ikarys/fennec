import { useEffect, useState, type ReactNode } from 'react';
import { Button } from '../components/ui/Button';
import { Tabs } from '../components/ui/Tabs';
import { TasksSection } from '../components/project/ideas/TasksSection';
import { MapSection } from '../components/project/MapSection';
import { DocSection } from '../components/project/DocSection';
import {
  createSession,
  getProjectOverview,
  type Project,
  type ProjectOverview,
} from '../lib/api';
import { frameIdea, type IdeaView } from '../lib/ideas';

// The idea's text is the human's; the framing asks for a plan, not code.
const framePrompt = (idea: string) =>
  `Frame this idea into a plan; ask me what is unclear before planning.\n\nIdea: ${idea}`;

const ANALYZE_PROMPT =
  'Analyze this project and draft the philosophy, layers and rules of its AGENTS.md.';

const TABS = [
  { id: 'tasks', label: 'Tâches' },
  { id: 'map', label: 'Carte des modules' },
  { id: 'doc', label: 'Doc' },
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

  // Unknown or null tab defaults to 'tasks'.
  const activeTab: TabId = TABS.find((t) => t.id === tab)?.id ?? 'tasks';

  const handleTabSelect = (id: TabId) => {
    window.location.hash = `#/project/${encodeURIComponent(projectId)}/${id}`;
  };

  const handleAnalyze = async () => {
    setAnalyzing(true);
    setAnalyzeError(null);
    try {
      const created = await createSession(projectId, ANALYZE_PROMPT, 'analyze');
      onOpenSession(created.id);
    } catch (err) {
      setAnalyzeError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setAnalyzing(false);
    }
  };

  // Creates the plan session, links it to the idea, and opens it.
  const handleFrame = async (idea: IdeaView) => {
    const session = await createSession(projectId, framePrompt(idea.text), 'plan');
    await frameIdea(projectId, idea.id, session.id);
    onOpenSession(session.id);
  };

  const shell = (children: ReactNode) => (
    <main className="grow overflow-y-auto px-6 py-8">
      <div className="mx-auto max-w-[1100px]">{children}</div>
    </main>
  );

  if (!project) {
    return shell(<p className="text-muted">Projet introuvable.</p>);
  }

  const header = (
    <>
      <div className="flex items-center gap-3">
        <h1 className="text-[1.375rem] font-semibold tracking-[-0.02em]">
          {project.name}
        </h1>
        <div className="grow" />
        <Button
          variant="secondary"
          size="sm"
          disabled={analyzing}
          onClick={handleAnalyze}
          title="Un agent en lecture seule rédige un brouillon d'AGENTS.md, chaque point avec sa source ; à relire"
        >
          {analyzing ? 'Création…' : 'Analyser le projet'}
        </Button>
      </div>
      {analyzeError ? <p className="mt-1 text-xs text-err">{analyzeError}</p> : null}
      <p className="mt-0.5 font-mono text-xs text-muted">
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
      <div className="mt-6 flex gap-6">
        <Tabs
          label="Sections du projet"
          tabs={[...TABS]}
          active={activeTab}
          onSelect={handleTabSelect}
        />
        <div className="min-w-0 grow">
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
          {activeTab === 'doc' ? <DocSection projectId={projectId} /> : null}
        </div>
      </div>
    </>,
  );
}
