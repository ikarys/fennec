import { useEffect, useState, type ReactNode } from 'react';
import { Button } from '../components/ui/Button';
import { IdeasSection } from '../components/project/IdeasSection';
import { MapSection } from '../components/project/MapSection';
import { DocSection } from '../components/project/DocSection';
import {
  createSession,
  getProjectOverview,
  type Project,
  type ProjectOverview,
} from '../lib/api';

// The idea's text is the human's; the framing asks for a plan, not code.
const framePrompt = (idea: string) =>
  `Frame this idea into a plan; ask me what is unclear before planning.\n\nIdea: ${idea}`;

const ANALYZE_PROMPT =
  'Analyze this project and draft the philosophy, layers and rules of its AGENTS.md.';

const TABS = [
  { id: 'ideas', label: 'Idées' },
  { id: 'map', label: 'Carte des modules' },
  { id: 'doc', label: 'Doc' },
] as const;

type TabId = (typeof TABS)[number]['id'];

export default function ProjectPage({
  projects,
  projectId,
  onOpenSession,
}: {
  projects: Project[];
  projectId: string;
  onOpenSession: (sessionId: string) => void;
}) {
  const project = projects.find((p) => p.id === projectId) ?? null;
  const [data, setData] = useState<ProjectOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);
  const [tab, setTab] = useState<TabId>('ideas');

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

  if (loading) {
    return shell(
      <>
        {header}
        <p className="mt-6 text-muted">Chargement…</p>
      </>,
    );
  }

  if (error) {
    return shell(
      <>
        {header}
        <p className="mt-6 text-err">{error}</p>
      </>,
    );
  }

  if (!data) {
    return shell(
      <>
        {header}
        <p className="mt-6 text-muted">Chargement…</p>
      </>,
    );
  }

  return shell(
    <>
      {header}
      <div className="mt-6 flex gap-6">
        <nav
          aria-label="Sections du projet"
          className="flex w-44 shrink-0 flex-col gap-1"
        >
          {TABS.map((candidate) => (
            <button
              key={candidate.id}
              type="button"
              onClick={() => setTab(candidate.id)}
              className={`rounded-md px-3 py-2 text-left text-sm transition-colors ${
                tab === candidate.id
                  ? 'bg-raised font-medium text-text'
                  : 'text-muted hover:bg-raised hover:text-text'
              }`}
            >
              {candidate.label}
            </button>
          ))}
        </nav>
        <div className="min-w-0 grow">
          {tab === 'ideas' ? (
            <IdeasSection
              projectId={projectId}
              onFrame={async (idea) =>
                (await createSession(projectId, framePrompt(idea.text), 'plan'))
                  .id
              }
              onOpenSession={onOpenSession}
            />
          ) : null}
          {tab === 'map' ? (
            <MapSection
              projectId={projectId}
              map={data.map}
              mapFile={data.mapFile}
              onWritten={(state) =>
                setData((prev) => (prev ? { ...prev, mapFile: state } : prev))
              }
            />
          ) : null}
          {tab === 'doc' ? <DocSection projectId={projectId} /> : null}
        </div>
      </div>
    </>,
  );
}
