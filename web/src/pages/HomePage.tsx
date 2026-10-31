import { useState } from 'react';
import type { Project, SessionSummary } from '../lib/api';
import { DeleteSessionsDialog, type DeletionTarget } from '../components/session/DeleteSessionsDialog';
import { formatRelative } from '../lib/format';
import { SessionStateIcon } from '../components/session/SessionStateIcon';
import { ChevronRightIcon, ProjectIcon, PlusIcon, TrashIcon } from '../components/Icons';
import { Button, buttonClass } from '../components/ui/Button';
import { Badge } from '../components/ui/Badge';
import { SectionLabel } from '../components/ui/SectionLabel';
import { Section } from '../components/ui/Section';

/** How many non-active sessions a project section shows before the toggle appears. */
const RECENT_SESSIONS = 5;

/** The states a session card keeps visible even when the list is collapsed. */
const ACTIVE_STATES: Set<string> = new Set(['running', 'waiting_for_user', 'error']);

function SessionRow({
  session,
  selected,
  onToggle,
  onDelete,
  onOpenSession
}: {
  session: SessionSummary;
  selected: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenSession: (id: string) => void;
}) {
  const running = session.state === 'running';
  return (
    <div className="group flex h-8 w-full items-center gap-3 border-b border-border px-3 last:border-b-0 hover:bg-raised">
      <input
        type="checkbox"
        className="h-3.5 w-3.5 shrink-0 accent-[var(--color-accent)]"
        aria-label={`Sélectionner ${session.title}`}
        checked={selected}
        disabled={running}
        title={running ? 'Une session en cours ne peut pas être supprimée' : undefined}
        onChange={() => onToggle(session.id)}
      />
      <button
        type="button"
        onClick={() => onOpenSession(session.id)}
        className="flex min-w-0 grow items-center gap-3 text-left"
      >
        <SessionStateIcon state={session.state} />
        <span className="min-w-0 grow truncate text-sm text-text">{session.title}</span>
        {session.awaitingIntegration && <Badge tone="accent">à intégrer</Badge>}
        <span className="w-24 shrink-0 whitespace-nowrap text-right font-mono text-[0.6875rem] text-muted tabular-nums">{formatRelative(session.updatedAt)}</span>
      </button>
      {!running && (
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-label="Supprimer la session"
          title="Supprimer la session"
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          onClick={() => onDelete(session.id)}
        >
          <TrashIcon className="h-3.5 w-3.5" />
        </Button>
      )}
    </div>
  );
}

function ProjectSection({
  project,
  defaultOpen,
  onNewSession,
  onOpenSession,
  selected,
  onToggle,
  onDelete
}: {
  project: Project;
  defaultOpen: boolean;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  selected: Set<string>;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const [expanded, setExpanded] = useState(false);
  // The server already returns the sessions newest first.
  const active = project.sessions.filter((session) => ACTIVE_STATES.has(session.state));
  const rest = project.sessions.filter((session) => !ACTIVE_STATES.has(session.state));
  const recent = rest.slice(0, RECENT_SESSIONS);
  const hidden = rest.slice(RECENT_SESSIONS);

  return (
    <Section
      flush
      title={
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open}
          className="flex items-center gap-2 text-left"
        >
          <ChevronRightIcon
            className={`h-3 w-3 shrink-0 text-muted transition-transform ${open ? 'rotate-90' : ''}`}
          />
          <span className="truncate">{project.name}</span>
        </button>
      }
      description={
        <span className="flex flex-wrap gap-3 font-mono">
          <span className="truncate">{project.workdir}</span>
          <span className="truncate">⎇ {project.branch ?? 'sans branche'}</span>
          <span className="tabular-nums">
            {project.sessions.length} session{project.sessions.length > 1 ? 's' : ''}
          </span>
        </span>
      }
      actions={
        <>
          <a
            href={'#/project/' + encodeURIComponent(project.id)}
            title="Voir le projet : tâches, carte des modules, doc"
            aria-label="Voir le projet"
            className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
          >
            <ProjectIcon className="h-4 w-4" />
          </a>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => onNewSession(project.id)}
            title="Nouvelle session dans ce projet"
            aria-label="Nouvelle session"
            iconOnly
          >
            <PlusIcon className="h-4 w-4" />
          </Button>
        </>
      }
    >

      {open &&
        (project.sessions.length === 0 ? (
          <div className="px-3 py-2 text-sm text-muted">Aucune session pour ce projet.</div>
        ) : (
          <div>
            {active.length > 0 && <SectionLabel className="px-3 pt-2 pb-1">En cours</SectionLabel>}
            {active.map((session) => (
              <SessionRow
                key={session.id}
                session={session}
                selected={selected.has(session.id)}
                onToggle={onToggle}
                onDelete={onDelete}
                onOpenSession={onOpenSession}
              />
            ))}
            {active.length > 0 && recent.length > 0 && <SectionLabel className="px-3 pt-2 pb-1">Récemment</SectionLabel>}
            {recent.map((session) => (
              <SessionRow
                key={session.id}
                session={session}
                selected={selected.has(session.id)}
                onToggle={onToggle}
                onDelete={onDelete}
                onOpenSession={onOpenSession}
              />
            ))}
            {rest.length > RECENT_SESSIONS && (
              <button
                type="button"
                onClick={() => setExpanded((value) => !value)}
                className="block w-full rounded-none px-3 py-1.5 text-left text-xs text-muted hover:bg-raised hover:text-text"
              >
                {expanded ? 'Replier' : `Afficher les ${hidden.length} autres sessions`}
              </button>
            )}
            {expanded &&
              hidden.map((session) => (
                <SessionRow
                  key={session.id}
                  session={session}
                  selected={selected.has(session.id)}
                  onToggle={onToggle}
                  onDelete={onDelete}
                  onOpenSession={onOpenSession}
                />
              ))}
          </div>
        ))}
    </Section>
  );
}

interface HomePageProps {
  projects: Project[];
  loading: boolean;
  error: string | null;
  onOpenProjectDialog: () => void;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
}

export function HomePage({ projects, loading, error, onOpenProjectDialog, onNewSession, onOpenSession, onSessionsDeleted }: HomePageProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<DeletionTarget[] | null>(null);
  const runningCount = projects.reduce(
    (count, project) => count + project.sessions.filter((session) => session.state === 'running').length,
    0
  );

  const toggleSession = (id: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const remove = (ids: string[]) => {
    const targets = projects.flatMap((project) =>
      project.sessions.filter((session) => ids.includes(session.id)).map((session) => ({ session, projectName: project.name }))
    );
    if (targets.length > 0) setPendingDelete(targets);
  };

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto flex max-w-[960px] flex-col gap-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="flex flex-col gap-0.5">
            <h1 className="text-xl font-semibold">Projets</h1>
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

        {selected.size > 0 && (
          <div className="sticky top-0 z-10 flex items-center gap-3 rounded-md border border-border bg-raised px-3 py-2">
            {selected.size === 1 ? '1 session sélectionnée' : `${selected.size} sessions sélectionnées`}
            <Button variant="danger" size="sm" onClick={() => remove([...selected])}>
              Supprimer ({selected.size})
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setSelected(new Set())}>
              Annuler
            </Button>
          </div>
        )}

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
            selected={selected}
            onToggle={toggleSession}
            onDelete={(id) => remove([id])}
          />
        ))}
      </div>

      {pendingDelete && (
        <DeleteSessionsDialog
          targets={pendingDelete}
          onClose={() => setPendingDelete(null)}
          onDeleted={(ids) => {
            onSessionsDeleted(ids);
            setSelected((previous) => {
              const next = new Set(previous);
              for (const id of ids) next.delete(id);
              return next;
            });
          }}
        />
      )}
    </main>
  );
}
