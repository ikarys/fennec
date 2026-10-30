/**
 * The project's ideas board: the queue, the stages, the night report.
 * The server computes everything; this only renders and polls.
 */
import { useCallback, useEffect, useState } from 'react';
import { NightReport } from './NightReport';
import { QueueBar } from './QueueBar';
import { StageGroup } from './StageGroup';
import {
  listIdeas,
  markReportRead,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';

export function IdeasSection({
  projectId,
  onFrame,
  onOpenSession,
}: {
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
}) {
  const [board, setBoard] = useState<IdeaBoard | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

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

  return (
    <section className="flex flex-col gap-4">
      {board.nightReport ? (
        <NightReport
          report={board.nightReport}
          onOpenSession={onOpenSession}
          onDismiss={() => run(() => markReportRead(projectId))}
        />
      ) : null}

      <QueueBar queue={board.queue} projectId={projectId} run={run} />

      {actionError ? <p className="text-xs text-err">{actionError}</p> : null}

      {board.groups.map((group) => (
        <StageGroup
          key={group.stage}
          stage={group.stage}
          ideas={group.ideas}
          projectId={projectId}
          onFrame={runFrame}
          onOpenSession={onOpenSession}
          run={run}
        />
      ))}
    </section>
  );
}
