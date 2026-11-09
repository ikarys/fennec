/**
 * The project's tasks board, split into horizontal tabs: an overview, the
 * human's queue, and the backlog's three server-computed views.
 * The server computes every figure; this component only renders and polls.
 */
import { useCallback, useEffect, useState } from 'react';
import { Tabs } from '../../ui/Tabs';
import { TasksOverview } from './TasksOverview';
import { QueueBar } from './QueueBar';
import { StageGroup } from './StageGroup';
import { NewIdeaDialog } from './NewIdeaDialog';
import { Button } from '../../ui/Button';
import { CloseIcon, PlusIcon } from '../../ui/Icons';
import {
  listIdeas,
  markReportRead,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';
import type { MsgKey } from '../../../i18n/messages';
import { useT } from '../../../i18n';

export type TasksSub = 'overview' | 'yours' | 'backlog' | 'ready' | 'done';

const TOP_TABS: readonly { id: TasksSub; labelKey: MsgKey }[] = [
  { id: 'overview', labelKey: 'tasksTabOverview' },
  { id: 'yours', labelKey: 'tasksTabYours' },
  { id: 'ready', labelKey: 'tasksTabReady' },
  { id: 'backlog', labelKey: 'tasksTabBacklog' },
  { id: 'done', labelKey: 'tasksTabDone' },
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
  const t = useT();

  // Every user action goes through this one path so failures are visible.
  const run = useCallback(async (action: () => Promise<IdeaBoard>) => {
    try {
      const next = await action();
      setBoard(next);
      setActionError(null);
      return true;
    } catch (err) {
      setActionError(err instanceof Error ? err.message : t('unknownError'));
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
        if (!cancelled) setLoadError(err instanceof Error ? err.message : t('unknownError'));
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
      <p className="text-muted">{t('loading')}</p>
    );
  }

  // The server sends every stage; the UI only picks the one the URL asks for.
  const yours = board.groups.find((g) => g.stage === 'yours')?.ideas ?? [];
  const drafts = board.groups.find((g) => g.stage === 'drafts')?.ideas ?? [];
  const queue = board.groups.find((g) => g.stage === 'queue')?.ideas ?? [];
  const done = board.groups.find((g) => g.stage === 'done')?.ideas ?? [];

  const activeSub = normalizeSub(sub);

  return (
    <section className="flex flex-col gap-4">
      <Tabs
        label={t('tasksSectionsLabel')}
        orientation="horizontal"
        tabs={TOP_TABS.map((tab) => ({
          id: tab.id,
          label: t(tab.labelKey),
          count:
            tab.id === 'yours'
              ? yours.length
              : tab.id === 'ready'
                ? queue.length
                : tab.id === 'backlog'
                  ? drafts.length
                  : tab.id === 'done'
                    ? done.length
                    : undefined,
        }))}
        active={activeSub}
        onSelect={(id) => onSelectSub(id as TasksSub)}
      />

      <div className="flex items-center gap-2">
        <div className="ml-auto">
          <Button
            variant="secondary"
            size="sm"
            title={t('addIdeaToBacklog')}
            onClick={() => setNewIdeaOpen(true)}
          >
            <PlusIcon className="h-4 w-4" />
            {t('newIdea')}
          </Button>
        </div>
      </div>

      {actionError ? (
        <div className="flex items-center gap-2">
          <p className="m-0 flex items-center gap-2 text-xs text-err" role="alert">
            {actionError}
          </p>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('close')}
            aria-label={t('close')}
            onClick={() => setActionError(null)}
          >
            <CloseIcon className="h-4 w-4" />
          </Button>
        </div>
      ) : null}

      {activeSub === 'overview' ? (
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

      {activeSub === 'yours' ? (
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
          <p className="text-sm text-muted">{t('nothingToDecide')}</p>
        )
      ) : null}

      {activeSub === 'ready' || activeSub === 'backlog' ? (
        <div className="flex flex-col gap-4">
          <QueueBar queue={board.queue} projectId={projectId} run={run} />

          {activeSub === 'backlog' && drafts.length === 0 ? (
            <p className="m-0 text-sm text-muted">{t('noDraftsPending')}</p>
          ) : null}
          {activeSub === 'ready' && queue.length === 0 ? (
            <p className="m-0 text-sm text-muted">{t('noQueuedFramed')}</p>
          ) : null}

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
        </div>
      ) : null}

      {activeSub === 'done' ? (
        done.length > 0 ? (
          <StageGroup
            stage="done"
            ideas={done}
            projectId={projectId}
            onFrame={runFrame}
            onOpenSession={onOpenSession}
            run={run}
            caps={board.queue.caps}
            showLabel={false}
          />
        ) : (
          <p className="m-0 text-sm text-muted">{t('noDoneTasks')}</p>
        )
      ) : null}

      <NewIdeaDialog
        projectId={projectId}
        open={newIdeaOpen}
        run={run}
        onClose={() => setNewIdeaOpen(false)}
        onCreated={() => onSelectSub('backlog')}
      />
    </section>
  );
}
