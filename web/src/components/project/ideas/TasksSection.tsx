/**
 * The project's tasks board: the queue, the stages, the night report.
 * The server computes everything; this only renders and polls.
 */
import { useCallback, useEffect, useState } from 'react';
import { Tabs } from '../../ui/Tabs';
import { NightReport } from './NightReport';
import { QueueBar } from './QueueBar';
import { StageGroup } from './StageGroup';
import { DoneTable } from './DoneTable';
import { AddIdeaField } from './AddIdeaField';
import { Button } from '../../ui/Button';
import { CloseIcon, PlusIcon } from '../../Icons';
import {
  listIdeas,
  markReportRead,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';

const SUB_TABS = [
  { id: 'backlog', label: 'Backlog' },
  { id: 'ready', label: 'Prêtes' },
  { id: 'done', label: 'Terminées' },
] as const;

type SubTabId = (typeof SUB_TABS)[number]['id'];

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
  onSelectSub: (sub: SubTabId) => void;
}) {
  const [board, setBoard] = useState<IdeaBoard | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // The add field stays folded until asked for.
  const [adding, setAdding] = useState(false);

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
  const running = board.groups.find((g) => g.stage === 'running')?.ideas ?? [];
  const drafts = board.groups.find((g) => g.stage === 'drafts')?.ideas ?? [];
  const queue = board.groups.find((g) => g.stage === 'queue')?.ideas ?? [];
  const done = board.groups.find((g) => g.stage === 'done')?.ideas ?? [];

  const activeSub: SubTabId = SUB_TABS.find((t) => t.id === sub)?.id ?? 'backlog';
  const counts: Record<SubTabId, number> = {
    backlog: drafts.length,
    ready: queue.length,
    done: done.length,
  };

  return (
    <section className="flex flex-col gap-4">
      {board.nightReport ? (
        <NightReport
          report={board.nightReport}
          caps={board.queue.caps}
          onOpenSession={onOpenSession}
          onDismiss={() => run(() => markReportRead(projectId))}
        />
      ) : null}

      <QueueBar queue={board.queue} projectId={projectId} run={run} />

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

      {yours.length > 0 ? (
        <StageGroup
          stage="yours"
          ideas={yours}
          projectId={projectId}
          onFrame={runFrame}
          onOpenSession={onOpenSession}
          run={run}
          caps={board.queue.caps}
        />
      ) : null}

      {running.length > 0 ? (
        <StageGroup
          stage="running"
          ideas={running}
          projectId={projectId}
          onFrame={runFrame}
          onOpenSession={onOpenSession}
          run={run}
          caps={board.queue.caps}
        />
      ) : null}

      {adding ? (
        <AddIdeaField projectId={projectId} run={run} onClose={() => setAdding(false)} />
      ) : null}

      <Tabs
        label="Sections des tâches"
        orientation="horizontal"
        tabs={SUB_TABS.map((t) => ({ id: t.id, label: t.label, count: counts[t.id] }))}
        active={activeSub}
        onSelect={onSelectSub}
        trailing={
          adding ? null : (
            <Button
              variant="soft"
              size="sm"
              iconOnly
              title="Ajouter une idée"
              aria-label="Ajouter une idée"
              onClick={() => setAdding(true)}
            >
              <PlusIcon className="h-4 w-4" />
            </Button>
          )
        }
      />

      {activeSub === 'backlog' ? (
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

      {activeSub === 'ready' ? (
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

      {activeSub === 'done' ? (
        <DoneTable ideas={done} onOpenSession={onOpenSession} />
      ) : null}
    </section>
  );
}
