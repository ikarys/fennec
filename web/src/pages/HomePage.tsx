import { useState } from 'react';
import type { Project, SessionSummary } from '../lib/api';
import { DeleteSessionsDialog, type DeletionTarget } from '../components/session/DeleteSessionsDialog';
import { formatRelative, formatDateTime } from '../lib/format';
import { SessionStateIcon } from '../components/session/SessionStateIcon';
import { BulbIcon, ChevronRightIcon, GlobeIcon, PlusIcon, TrashIcon } from '../components/Icons';
import { Button, buttonClass } from '../components/ui/Button';
import { WorktreeAlert } from '../components/project/WorktreeAlert';
import { Badge } from '../components/ui/Badge';
import { SectionLabel } from '../components/ui/SectionLabel';

/** How many non-active sessions a project section shows before the toggle appears. */
const RECENT_SESSIONS = 5;

/** The states a session card keeps visible even when the list is collapsed. */
const ACTIVE_STATES: Set<string> = new Set(['running', 'waiting_for_user', 'error']);

function SessionRow({
  session,
  selected,
  selecting,
  onToggle,
  onDelete,
  onOpenSession
}: {
  session: SessionSummary;
  selected: boolean;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenSession: (id: string) => void;
}) {
  const running = session.state === 'running';
  // The checkbox stays hidden until the row is hovered/focused or a selection exists.
  const hidden = selecting || selected ? '' : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100';

  return (
    <div className="group flex h-[34px] w-full items-center gap-3 border-t border-border px-3 hover:bg-raised">
      <input
        type="checkbox"
        className={`h-3.5 w-3.5 shrink-0 accent-[var(--color-accent)] ${hidden}`}
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
        {session.awaitingIntegration && <Badge tone="accent" solid>à intégrer</Badge>}
        <span title={formatDateTime(session.createdAt)} className="w-24 shrink-0 whitespace-nowrap text-right font-mono text-xs text-faint tabular-nums">
          {formatRelative(session.updatedAt)}
        </span>
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

/** The shared session list: the active/recent split, the expand toggle and the empty state. */
function SessionList({
  sessions,
  selected,
  selecting,
  onToggle,
  onDelete,
  onOpenSession,
  empty,
  noun,
  expanded,
  onExpandedChange
}: {
  sessions: SessionSummary[];
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenSession: (id: string) => void;
  empty: string;
  noun: string;
  expanded: boolean;
  onExpandedChange: (value: boolean) => void;
}) {
  // The server already returns the sessions newest first.
  const active = sessions.filter((session) => ACTIVE_STATES.has(session.state));
  const rest = sessions.filter((session) => !ACTIVE_STATES.has(session.state));
  const recent = rest.slice(0, RECENT_SESSIONS);
  const hidden = rest.slice(RECENT_SESSIONS);

  if (sessions.length === 0) {
    return (
      <div className="border-t border-border px-3 py-2.5 text-sm text-muted">{empty}</div>
    );
  }

  return (
    <div>
      {active.length > 0 && <SectionLabel className="px-3 pt-2 pb-1">En cours</SectionLabel>}
      {active.map((session) => (
        <SessionRow
          key={session.id}
          session={session}
          selected={selected.has(session.id)}
          selecting={selecting}
          onToggle={onToggle}
          onDelete={onDelete}
          onOpenSession={onOpenSession}
        />
      ))}
      {active.length > 0 && recent.length > 0 && (
        <SectionLabel className="px-3 pt-2 pb-1">Récemment</SectionLabel>
      )}
      {recent.map((session) => (
        <SessionRow
          key={session.id}
          session={session}
          selected={selected.has(session.id)}
          selecting={selecting}
          onToggle={onToggle}
          onDelete={onDelete}
          onOpenSession={onOpenSession}
        />
      ))}
      {rest.length > RECENT_SESSIONS && (
        <button
          type="button"
          onClick={() => onExpandedChange(!expanded)}
          className="block w-full border-t border-border px-3 py-[7px] text-left text-xs text-faint hover:bg-raised hover:text-text"
        >
          {expanded ? 'Replier' : `Afficher les ${hidden.length} autres ${noun}`}
        </button>
      )}
      {expanded &&
        hidden.map((session) => (
          <SessionRow
            key={session.id}
            session={session}
            selected={selected.has(session.id)}
            selecting={selecting}
            onToggle={onToggle}
            onDelete={onDelete}
            onOpenSession={onOpenSession}
          />
        ))}
    </div>
  );
}

function ProjectSection({
  project,
  defaultOpen,
  onNewSession,
  onOpenSession,
  selected,
  selecting,
  onToggle,
  onDelete
}: {
  project: Project;
  defaultOpen: boolean;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const [open, setOpen] = useState(defaultOpen);
  // The expand state lives here, not in SessionList, so it survives collapsing
  // and reopening the section.
  const [expanded, setExpanded] = useState(false);
  // The live badge only appears while this project has running sessions.
  const runningCount = project.sessions.filter((session) => session.state === 'running').length;

  return (
    <section className="overflow-hidden rounded-md border border-border bg-panel">
      <div className="flex min-h-12 items-center gap-2.5 px-3 py-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
          className="flex min-w-0 grow items-center gap-2.5 text-left"
        >
          <ChevronRightIcon
            className={`h-3 w-3 shrink-0 text-muted transition-transform ${open ? 'rotate-90' : ''}`}
          />
          <span className="shrink-0 text-sm font-semibold">{project.name}</span>
          {runningCount > 0 && (
            <Badge tone="accent" solid dot pulse>
              {runningCount} en cours
            </Badge>
          )}
          {project.sessionWorktree && (
            <Badge tone="err" title="Ce dossier est l'espace de travail d'une session de fennec : une intégration ou un nettoyage peut le supprimer.">
              worktree d'une session
            </Badge>
          )}
          <span className="ml-1 flex min-w-0 gap-3 font-mono text-xs text-faint">
            <span className="truncate whitespace-nowrap">{project.workdir}</span>
            <span className="truncate whitespace-nowrap">⎇ {project.branch ?? 'sans branche'}</span>
            <span className="truncate whitespace-nowrap tabular-nums">
              {project.sessions.length} session{project.sessions.length > 1 ? 's' : ''}
            </span>
          </span>
        </button>
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <a
            href={'#/project/' + encodeURIComponent(project.id)}
            title="Voir le projet : tâches, carte des modules, doc"
            aria-label="Voir le projet"
            className={buttonClass({ variant: 'ghost', size: 'sm' })}
          >
            <GlobeIcon className="h-3.5 w-3.5" />
            Projet
          </a>
          <Button
            variant="ghost"
            size="sm"
            className="text-accent hover:bg-accent-soft hover:text-accent-strong"
            onClick={() => onNewSession(project.id)}
            title="Nouvelle session dans ce projet"
            aria-label="Nouvelle session"
          >
            <PlusIcon className="h-3.5 w-3.5" />
            Session
          </Button>
          <a
            href={'#/project/' + encodeURIComponent(project.id) + '/tasks'}
            title="Voir les tâches du projet"
            aria-label="Tâches du projet"
            className={buttonClass({ variant: 'ghost', size: 'sm' })}
          >
            <BulbIcon className="h-3.5 w-3.5" />
            Tâches
          </a>
        </div>
      </div>

      {open && (
        <SessionList
          sessions={project.sessions}
          selected={selected}
          selecting={selecting}
          onToggle={onToggle}
          onDelete={onDelete}
          onOpenSession={onOpenSession}
          empty="Aucune session pour ce projet."
          noun="sessions"
          expanded={expanded}
          onExpandedChange={setExpanded}
        />
      )}
    </section>
  );
}

/** The chat: conversations with no project, folder or git. */
function ChatSection({
  project,
  onNewSession,
  onOpenSession,
  selected,
  selecting,
  onToggle,
  onDelete
}: {
  project: Project;
  onNewSession: () => void;
  onOpenSession: (sessionId: string) => void;
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  return (
    <section className="overflow-hidden rounded-md border border-border bg-panel">
      <div className="flex min-h-12 items-center gap-2.5 px-3 py-2">
        <span className="truncate text-sm font-semibold">Chat</span>
        <span className="ml-1 flex min-w-0 gap-3 font-mono text-xs text-faint">
          <span className="truncate whitespace-nowrap tabular-nums">
            {project.sessions.length} conversation{project.sessions.length > 1 ? 's' : ''}
          </span>
        </span>
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <Button
            variant="ghost"
            size="sm"
            className="text-accent hover:bg-accent-soft hover:text-accent-strong"
            onClick={onNewSession}
            title="Nouvelle conversation sans projet, dossier ni git"
            aria-label="Nouveau chat"
          >
            <PlusIcon className="h-3.5 w-3.5" />
            Nouveau chat
          </Button>
        </div>
      </div>

      <SessionList
        sessions={project.sessions}
        selected={selected}
        selecting={selecting}
        onToggle={onToggle}
        onDelete={onDelete}
        onOpenSession={onOpenSession}
        empty="Aucune conversation. « Nouveau chat » pour parler sans projet."
        noun="conversations"
        expanded={expanded}
        onExpandedChange={setExpanded}
      />
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
  onSessionsDeleted: (ids: string[]) => void;
}

export function HomePage({
  projects,
  loading,
  error,
  onOpenProjectDialog,
  onNewSession,
  onOpenSession,
  onSessionsDeleted
}: HomePageProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<DeletionTarget[] | null>(null);
  const runningCount = projects.reduce(
    (count, project) => count + project.sessions.filter((session) => session.state === 'running').length,
    0
  );
  // The chat is a scratch project: it gets its own section, not a project one.
  const chatProject = projects.find((project) => project.chat);
  const regularProjects = projects.filter((project) => !project.chat);

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
      project.sessions
        .filter((session) => ids.includes(session.id))
        .map((session) => ({ session, projectName: project.name }))
    );
    if (targets.length > 0) setPendingDelete(targets);
  };

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto flex max-w-[960px] flex-col gap-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="flex flex-col gap-0.5">
            <h1 className="text-2xl font-semibold">Projets</h1>
            <p className="flex items-center gap-2 text-sm text-muted">
              {runningCount > 0 && <span className="h-2 w-2 animate-pulse rounded-full bg-accent" />}
              {runningCount > 0
                ? `${runningCount} session${runningCount > 1 ? 's' : ''} en cours`
                : 'Aucune session en cours'}
            </p>
            <WorktreeAlert />
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

        {!loading && !error && regularProjects.length === 0 && (
          <p className="m-0 py-10 text-center text-muted">
            Aucun projet ouvert. Ouvre un dossier pour commencer.
          </p>
        )}

        {chatProject && (
          <ChatSection
            project={chatProject}
            onNewSession={() => onNewSession(chatProject.id)}
            onOpenSession={onOpenSession}
            selected={selected}
            selecting={selected.size > 0}
            onToggle={toggleSession}
            onDelete={(id) => remove([id])}
          />
        )}

        {/* The server lists the most recently active project first: it opens unfolded. */}
        {regularProjects.map((project, index) => (
          <ProjectSection
            key={project.id}
            project={project}
            defaultOpen={index === 0}
            onNewSession={onNewSession}
            onOpenSession={onOpenSession}
            selected={selected}
            selecting={selected.size > 0}
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
