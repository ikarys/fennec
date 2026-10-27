import type { Project } from '../lib/api';
import { formatRelative, stateDotClass, stateLabel, stateTextClass } from '../lib/format';
import { PlusIcon } from '../components/Icons';

interface HomePageProps {
  projects: Project[];
  loading: boolean;
  error: string | null;
  onOpenProjectDialog: () => void;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
}

export function HomePage({ projects, loading, error, onOpenProjectDialog, onNewSession, onOpenSession }: HomePageProps) {
  const runningCount = projects.reduce(
    (count, project) => count + project.sessions.filter((session) => session.state === 'running').length,
    0
  );

  return (
    <main className="grow overflow-y-auto px-6 py-8">
      <div className="mx-auto flex max-w-[960px] flex-col gap-7">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h1 className="text-[1.375rem] font-semibold tracking-[-0.02em]">Projets</h1>
            <p className="mt-0.5 text-muted">
              {runningCount > 0 ? `${runningCount} session${runningCount > 1 ? 's' : ''} en cours` : 'Aucune session en cours'}
            </p>
          </div>
          <button
            type="button"
            onClick={onOpenProjectDialog}
            className="flex min-h-9 items-center gap-2 rounded-lg border border-border bg-raised px-3.5 text-[0.8125rem] font-medium text-text"
          >
            <svg
              className="h-[15px] w-[15px]"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
            </svg>
            Ouvrir un projet
          </button>
        </div>

        {error && <div className="rounded-lg border border-border bg-raised px-4 py-3 text-sm text-err">{error}</div>}

        {loading && projects.length === 0 && <div className="text-sm text-muted">Chargement…</div>}

        {!loading && !error && projects.length === 0 && (
          <div className="rounded-xl border border-border bg-panel px-6 py-10 text-center text-sm text-muted">
            Aucun projet ouvert. Ouvre un dossier pour commencer.
          </div>
        )}

        {projects.map((project) => (
          <section key={project.id} className="overflow-hidden rounded-xl border border-border bg-panel">
            <div className="flex flex-wrap items-center gap-3 border-b border-border px-4 py-3.5">
              <div className="min-w-0 grow">
                <h2 className="text-[0.9375rem] font-semibold">{project.name}</h2>
                <div className="mt-1 flex flex-wrap gap-3 text-xs text-muted">
                  <span className="font-mono">{project.workdir}</span>
                  <span className="font-mono">⎇ {project.branch ?? 'sans branche'}</span>
                </div>
              </div>
              <button
                type="button"
                onClick={() => onNewSession(project.id)}
                className="flex min-h-[34px] items-center gap-1.5 rounded-lg bg-accent px-3 text-[0.8125rem] font-medium text-on-accent"
              >
                <PlusIcon className="h-3.5 w-3.5" />
                Nouvelle session
              </button>
            </div>

            <div className="overflow-x-auto">
              {project.sessions.length === 0 && (
                <div className="px-4 py-4 text-sm text-muted">Aucune session pour ce projet.</div>
              )}
              {project.sessions.map((session) => (
                <button
                  key={session.id}
                  type="button"
                  onClick={() => onOpenSession(session.id)}
                  className="flex min-h-11 min-w-[560px] items-center gap-3 border-t border-border px-4 text-left text-text first:border-t-0"
                >
                  <span className={`h-[7px] w-[7px] flex-none rounded-full ${stateDotClass(session.state)}`} />
                  <span className="min-w-0 grow overflow-hidden text-ellipsis whitespace-nowrap font-medium">
                    {session.title}
                  </span>
                  <span className={`w-[92px] text-xs ${stateTextClass(session.state)}`}>{stateLabel(session.state)}</span>
                  <span className="w-24 text-right text-xs text-muted">{formatRelative(session.updatedAt)}</span>
                </button>
              ))}
            </div>
          </section>
        ))}
      </div>
    </main>
  );
}
