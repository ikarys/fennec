/**
 * The tasks overview: four dashboard tiles, the always-visible queue bar,
 * the last night report, and the ideas that are currently running.
 * The server computes the figures; this component only renders them.
 */
import { Tile } from '../../ui/Tile';
import { QueueBar } from './QueueBar';
import { NightReport } from './NightReport';
import { StageGroup } from './StageGroup';
import { ReviewIcon, PlayIcon, BulbIcon, CheckIcon } from '../../ui/Icons';
import type { TasksSub } from './TasksSection';
import type { IdeaBoard, IdeaView } from '../../../lib/ideas';
import { useT } from '../../../i18n';

export function TasksOverview({
  board,
  projectId,
  onFrame,
  onOpenSession,
  run,
  onSelect,
  onDismissReport
}: {
  board: IdeaBoard;
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  onSelect: (sub: TasksSub) => void;
  onDismissReport: () => Promise<boolean>;
}) {
  const yours = board.groups.find((g) => g.stage === 'yours')?.ideas ?? [];
  const running = board.groups.find((g) => g.stage === 'running')?.ideas ?? [];
  const done = board.groups.find((g) => g.stage === 'done')?.ideas ?? [];
  const summary = board.summary;
  const t = useT();

  const attentionParts: string[] = [];
  if (summary.review > 0) attentionParts.push(t('attentionToReview', { count: summary.review }));
  if (summary.question > 0) {
    attentionParts.push(
      t('attentionQuestions', { count: summary.question, s: summary.question > 1 ? 's' : '' })
    );
  }
  const attentionCount = summary.halted + summary.failed;
  if (attentionCount > 0) {
    attentionParts.push(
      t('attentionHalted', { count: attentionCount, s: attentionCount > 1 ? 's' : '' })
    );
  }
  const yoursDetail = attentionParts.join(' · ');

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Tile
          icon={<ReviewIcon />}
          figure={yours.length}
          label={t('tasksTabYours')}
          detail={yoursDetail}
          tone="accent"
          active={yours.length > 0}
          onClick={() => onSelect('yours')}
        />
        <Tile
          icon={<PlayIcon />}
          figure={running.length}
          label={t('stageRunning')}
          detail={
            board.queue.running
              ? board.queue.autonomous
                ? t('queueRunningNight')
                : t('queueRunning')
              : t('queueStopped')
          }
          tone="accent"
          active={running.length > 0}
          pulse={board.queue.running}
        />
        <Tile
          icon={<BulbIcon />}
          figure={board.queue.ready}
          label={t('tasksTabReady')}
          detail={board.queue.next ? t('nextLabel', { title: board.queue.next.title }) : t('nothingFramed')}
          tone="neutral"
          active={board.queue.ready > 0}
          title={board.queue.next?.text}
          onClick={() => onSelect('ready')}
        />
        <Tile
          icon={<CheckIcon />}
          figure={done.length}
          label={t('tasksTabDone')}
          detail={t('integratedCount', {
            count: summary.integrated,
            s: summary.integrated > 1 ? 's' : ''
          })}
          tone="ok"
          active={done.length > 0}
          onClick={() => onSelect('done')}
        />
      </div>

      <QueueBar queue={board.queue} projectId={projectId} run={run} />

      {board.nightReport ? (
        <NightReport
          report={board.nightReport}
          caps={board.queue.caps}
          onOpenSession={onOpenSession}
          onDismiss={onDismissReport}
        />
      ) : null}

      {running.length > 0 ? (
        <StageGroup
          stage="running"
          ideas={running}
          projectId={projectId}
          onFrame={onFrame}
          onOpenSession={onOpenSession}
          run={run}
          caps={board.queue.caps}
        />
      ) : null}
    </div>
  );
}
