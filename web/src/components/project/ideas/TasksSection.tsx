/**
 * The project's tasks board, split into horizontal tabs: an overview, the
 * human's queue, and the backlog's three server-computed views.
 * The server computes every figure; this component only renders and polls.
 */
import { useCallback, useEffect, useState } from 'react';
import { Tabs } from '../../ui/Tabs';
import { Segmented } from '../../ui/Segmented';
import { TasksOverview } from './TasksOverview';
import { QueueBar } from './QueueBar';
import { StageGroup } from './StageGroup';
import { DoneTable } from './DoneTable';
import { NewIdeaDialog } from './NewIdeaDialog';
import { Button } from '../../ui/Button';
import { CloseIcon } from '../../Icons';
import {
  listIdeas,
  markReportRead,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';

export type TasksSub = 'overview' | 'yours' | 'backlog' | 'ready' | 'done';

type TopTabId = 'overview' | 'yours' | 'queue';
type BacklogView = 'backlog' | 'ready' | 'done';

const TOP_TABS: readonly { id: TopTabId; label: string }[] = [
  { id: 'overview', label: 'Vue d’ensemble' },
  { id: 'yours', label: 'À décider' },
  { id: 'queue', label: 'Backlog' },
];

function normalizeSub(sub: string | null): TasksSub {
  if (
    sub === 'overview' ||
    sub === 'yours' ||
    sub === 'backlog' ||
    sub === 'ready' ||
    sub === 'done'
  ) {
    return sub;
  }
  return 'overview';
}

function topTabFor(sub: TasksSub): TopTabId {
  if (sub === 'overview') return 'overview';
  if (sub === 'yours') return 'yours';
  return 'queue';
}

function backlogViewFor(sub: TasksSub): BacklogView {
  if (sub === 'ready' || sub === 'done') return sub;
  return 'backlog';
}

export function TasksSection({
  projectId,
  sub,
  onFrame,
  onOpenSession,
  onSelectSub,
}: {
  projectId: string;
  sub: string | null;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  onSelectSub: (sub: TasksSub) => void;
}) {
  const [board, setBoard] = useState<IdeaBoard | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // The add dialog stays closed until asked for.
  const [newIdeaOpen, setNewIdeaOpen] = useState(false);

  // Every user action goes through this one path so failures are visible.
  const run = useCallback(async (action: () => Promise<IdeaBoard>) => {
    try {
      const next = await action();
      setBoard(next);
      setActionError(null);
      return true;
    } catch (err) {
      setActionError(err instanceof Error ? err.message : 'Erreur inconnue');
      return false;
    }
  }, []);

  // Framing does not return a board; refresh after the parent's action so the
  // same run() path can report success or failure.
  const runFrame = useCallback(
    async (idea: IdeaView) => {
      await run(async () => {
        await onFrame(idea);
        return listIdeas(projectId);
      });
    },
    [onFrame, projectId, run],
  );

  useEffect(() => {
    let cancelled = false;
    setBoard(null);
    setLoadError(null);
    setActionError(null);

    listIdeas(projectId)
      .then((b) => {
        if (!cancelled) {
          setBoard(b);
          setLoadError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) setLoadError(err instanceof Error ? err.message : 'Erreur inconnue');
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  // Poll only while the queue is running or any idea is in 'running' status.
  const polling =
    board !== null &&
    (board.queue.running ||
      board.groups.some((g) => g.ideas.some((i) => i.status === 'running')));

  useEffect(() => {
    if (!polling) return;
    const id = window.setInterval(() => {
      listIdeas(projectId)
        .then((b) => setBoard(b))
        .catch(() => {
          // Keep the last known board on a transient poll failure.
        });
    }, 5000);
    return () => window.clearInterval(id);
  }, [polling, projectId]);

  if (!board) {
    return loadError ? (
      <p className="text-xs text-err">{loadError}</p>
    ) : (
      <p className="text-muted">Chargement…</p>
    );
  }

  // The server sends every stage; the UI only picks the one the URL asks for.
  const yours = board.groups.find((g) => g.stage === 'yours')?.ideas ?? [];
  const drafts = board.groups.find((g) => g.stage === 'drafts')?.ideas ?? [];
  const queue = board.groups.find((g) => g.stage === 'queue')?.ideas ?? [];
  const done = board.groups.find((g) => g.stage === 'done')?.ideas ?? [];

  const activeSub = normalizeSub(sub);
  const activeTopTab = topTabFor(activeSub);
  const backlogView = backlogViewFor(activeSub);

  return (
    <section className="flex flex-col gap-4">
      <Tabs
        label="Sections des tâches"
        orientation="horizontal"
        tabs={TOP_TABS.map((t) => ({
          id: t.id,
          label: t.label,
          count:
            t.id === 'yours'
              ? yours.length
              : t.id === 'queue'
                ? drafts.length + queue.length
                : undefined,
        }))}
        active={activeTopTab}
        onSelect={(id) => {
          const tab = id as TopTabId;
          if (tab === 'queue') {
            onSelectSub('backlog');
          } else {
            onSelectSub(tab);
          }
        }}
      />

      {actionError ? (
        <div className="flex items-center gap-2">
          <p className="m-0 flex items-center gap-2 text-xs text-err" role="alert">
            {actionError}
          </p>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title="Fermer"
            aria-label="Fermer"
            onClick={() => setActionError(null)}
          >
            <CloseIcon className="h-4 w-4" />
          </Button>
        </div>
      ) : null}

      {activeTopTab === 'overview' ? (
        <TasksOverview
          board={board}
          projectId={projectId}
          onFrame={runFrame}
          onOpenSession={onOpenSession}
          run={run}
          onSelect={onSelectSub}
          onDismissReport={() => run(() => markReportRead(projectId))}
        />
      ) : null}

      {activeTopTab === 'yours' ? (
        yours.length > 0 ? (
          <StageGroup
            stage="yours"
            ideas={yours}
            projectId={projectId}
            onFrame={runFrame}
            onOpenSession={onOpenSession}
            run={run}
            caps={board.queue.caps}
            showLabel={false}
          />
        ) : (
          <p className="text-sm text-muted">Rien ne t’attend.</p>
        )
      ) : null}

      {activeTopTab === 'queue' ? (
        <div className="flex flex-col gap-4">
          <QueueBar queue={board.queue} projectId={projectId} run={run} />

          <div className="flex items-center gap-2">
            <Segmented
              label="Vue du backlog"
              segments={[
                { value: 'backlog' as const, label: `Idées (${drafts.length})` },
                { value: 'ready' as const, label: `Prêtes (${queue.length})` },
                { value: 'done' as const, label: `Terminées (${done.length})` },
              ]}
              value={backlogView}
              onChange={(view) => onSelectSub(view as BacklogView)}
            />
            <div className="ml-auto">
              <Button
                variant="secondary"
                size="sm"
                title="Ajouter une idée"
                onClick={() => setNewIdeaOpen(true)}
              >
                + Add task
              </Button>
            </div>
          </div>

          {backlogView === 'backlog' && drafts.length === 0 ? (
            <p className="m-0 text-sm text-muted">Aucune idée en attente de cadrage.</p>
          ) : null}
          {backlogView === 'ready' && queue.length === 0 ? (
            <p className="m-0 text-sm text-muted">Aucune idée cadrée dans la file.</p>
          ) : null}

          {backlogView === 'backlog' ? (
            <StageGroup
              stage="drafts"
              ideas={drafts}
              projectId={projectId}
              onFrame={runFrame}
              onOpenSession={onOpenSession}
              run={run}
              caps={board.queue.caps}
              showLabel={false}
            />
          ) : null}

          {backlogView === 'ready' ? (
            <StageGroup
              stage="queue"
              ideas={queue}
              projectId={projectId}
              onFrame={runFrame}
              onOpenSession={onOpenSession}
              run={run}
              caps={board.queue.caps}
              showLabel={false}
            />
          ) : null}

          {backlogView === 'done' ? (
            <DoneTable ideas={done} onOpenSession={onOpenSession} />
          ) : null}
        </div>
      ) : null}

      <NewIdeaDialog
        projectId={projectId}
        open={newIdeaOpen}
        run={run}
        onClose={() => setNewIdeaOpen(false)}
      />
    </section>
  );
}
