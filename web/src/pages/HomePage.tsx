import { useState } from 'react';
import type { Project, SessionSummary } from '../lib/api';
import { formatRelative, stateLabel } from '../lib/format';
import { ChevronRightIcon, PlusIcon } from '../components/Icons';
import { Button, buttonClass } from '../components/ui/Button';
import { Badge, type BadgeTone } from '../components/ui/Badge';
import { SectionLabel } from '../components/ui/SectionLabel';

/** How many non-active sessions a project section shows before the toggle appears. */
const RECENT_SESSIONS = 5;

/** The states a session card keeps visible even when the list is collapsed. */
const ACTIVE_STATES: Set<string> = new Set(['running', 'waiting_for_user', 'error']);

function sessionTone(state: SessionSummary['state']): { tone: BadgeTone; dot: boolean; pulse: boolean } {
  if (state === 'running') return { tone: 'accent', dot: true, pulse: true };
  if (state === 'waiting_for_user') return { tone: 'accent', dot: true, pulse: false };
  if (state === 'error') return { tone: 'err', dot: false, pulse: false };
  if (state === 'complete') return { tone: 'ok', dot: false, pulse: false };
  return { tone: 'neutral', dot: false, pulse: false };
}

function SessionRow({ session, onOpenSession }: { session: SessionSummary; onOpenSession: (id: string) => void }) {
  const { tone, dot, pulse } = sessionTone(session.state);
  return (
    <button
      type="button"
      onClick={() => onOpenSession(session.id)}
      className="group flex h-8 w-full items-center gap-3 border-b border-border px-3 text-left hover:bg-raised"
    >
      <span className="min-w-0 grow truncate text-sm text-text">{session.title}</span>
      <span className="flex w-40 shrink-0 items-center gap-1.5 font-mono text-[0.6875rem] text-muted tabular-nums">
        {session.awaitingIntegration && <Badge tone="accent">à intégrer</Badge>}
        <Badge tone={tone} dot={dot} pulse={pulse}>
          {stateLabel(session.state)}
        </Badge>
      </span>
      <span className="w-24 shrink-0 text-right font-mono text-[0.6875rem] text-muted tabular-nums">{formatRelative(session.updatedAt)}</span>
    </button>
  );
}

function ProjectSection({
  project,
  defaultOpen,
  onNewSession,
  onOpenSession
}: {
  project: Project;
  defaultOpen: boolean;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const [expanded, setExpanded] = useState(false);
  // The server already returns the sessions newest first.
  const active = project.sessions.filter((session) => ACTIVE_STATES.has(session.state));
  const rest = project.sessions.filter((session) => !ACTIVE_STATES.has(session.state));
  const recent = rest.slice(0, RECENT_SESSIONS);
  const hidden = rest.slice(RECENT_SESSIONS);

  return (
    <section className="border-t border-border pt-3 first:border-t-0 first:pt-0">
      <div className="group flex items-center gap-3 border-b border-border px-3 hover:bg-raised">
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open}
          className="flex min-w-0 flex-1 flex-col gap-1 rounded text-left"
        >
          <span className="flex items-center gap-2">
            <ChevronRightIcon
              className={`h-3 w-3 shrink-0 text-muted transition-transform ${open ? 'rotate-90' : ''}`}
            />
            <span className="truncate text-sm font-semibold">{project.name}</span>
          </span>
          <span className="flex flex-wrap gap-3 text-xs font-mono text-muted">
            <span className="truncate">{project.workdir}</span>
            <span className="truncate">⎇ {project.branch ?? 'sans branche'}</span>
            <span className="tabular-nums">
              {project.sessions.length} session{project.sessions.length > 1 ? 's' : ''}
            </span>
          </span>
        </button>
        <span className="flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100">
          <a
            href={'#/project/' + encodeURIComponent(project.id)}
            title="La carte du projet, ses ADR"
            className={buttonClass({ variant: 'ghost', size: 'sm' })}
          >
            Projet
          </a>
          <Button variant="secondary" size="sm" onClick={() => onNewSession(project.id)}>
            <PlusIcon className="h-3.5 w-3.5" />
            Nouvelle session
          </Button>
        </span>
      </div>

      {open &&
        (project.sessions.length === 0 ? (
          <div className="px-2 py-2 text-muted">Aucune session pour ce projet.</div>
        ) : (
          <div>
            {active.length > 0 && <SectionLabel className="px-2 pt-2 pb-1">En cours</SectionLabel>}
            {active.map((session) => (
              <SessionRow key={session.id} session={session} onOpenSession={onOpenSession} />
            ))}
            {active.length > 0 && recent.length > 0 && <SectionLabel className="px-2 pt-2 pb-1">Récemment</SectionLabel>}
            {recent.map((session) => (
              <SessionRow key={session.id} session={session} onOpenSession={onOpenSession} />
            ))}
            {rest.length > RECENT_SESSIONS && (
              <button
                type="button"
                onClick={() => setExpanded((value) => !value)}
                className="block w-full rounded-md px-2 py-1.5 text-left text-xs text-muted hover:bg-raised hover:text-text"
              >
                {expanded ? 'Replier' : `Afficher les ${hidden.length} autres sessions`}
              </button>
            )}
            {expanded &&
              hidden.map((session) => (
                <SessionRow key={session.id} session={session} onOpenSession={onOpenSession} />
              ))}
          </div>
        ))}
    </section>
  );
}

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
      <div className="mx-auto flex max-w-[960px] flex-col gap-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="flex flex-col gap-0.5">
            <h1 className="text-[0.9375rem] font-semibold">Projets</h1>
            <p className="text-sm text-muted">
              {runningCount > 0 ? `${runningCount} session${runningCount > 1 ? 's' : ''} en cours` : 'Aucune session en cours'}
            </p>
          </div>
          <Button variant="primary" size="md" onClick={onOpenProjectDialog}>
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
          </Button>
        </div>

        {error && <p className="m-0 text-err">{error}</p>}

        {loading && projects.length === 0 && <div className="text-sm text-muted">Chargement…</div>}

        {!loading && !error && projects.length === 0 && (
          <p className="m-0 py-10 text-center text-muted">
            Aucun projet ouvert. Ouvre un dossier pour commencer.
          </p>
        )}

        {/* The server lists the most recently active project first: it opens unfolded. */}
        {projects.map((project, index) => (
          <ProjectSection
            key={project.id}
            project={project}
            defaultOpen={index === 0}
            onNewSession={onNewSession}
            onOpenSession={onOpenSession}
          />
        ))}
      </div>
    </main>
  );
}
