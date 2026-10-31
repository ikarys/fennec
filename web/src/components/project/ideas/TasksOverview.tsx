/**
 * The tasks overview: four dashboard tiles, the always-visible queue bar,
 * the last night report, and the ideas that are currently running.
 * The server computes the figures; this component only renders them.
 */
import { Tile } from '../../ui/Tile';
import { QueueBar } from './QueueBar';
import { NightReport } from './NightReport';
import { StageGroup } from './StageGroup';
import { ReviewIcon, PlayIcon, BulbIcon, CheckIcon } from '../../Icons';
import type { TasksSub } from './TasksSection';
import type { IdeaBoard, IdeaView } from '../../../lib/ideas';

function singularForm(count: number, singular: string, plural: string): string {
  return count === 0 || count === 1 ? singular : plural;
}

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

  const attentionParts: string[] = [];
  if (summary.review > 0) attentionParts.push(`${summary.review} à relire`);
  if (summary.question > 0) {
    attentionParts.push(`${summary.question} ${singularForm(summary.question, 'question', 'questions')}`);
  }
  const attentionCount = summary.halted + summary.failed;
  if (attentionCount > 0) {
    attentionParts.push(`${attentionCount} ${singularForm(attentionCount, 'arrêtée', 'arrêtées')}`);
  }
  const yoursDetail = attentionParts.join(' · ');

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Tile
          icon={<ReviewIcon />}
          figure={yours.length}
          label="À décider"
          detail={yoursDetail}
          tone="accent"
          active={yours.length > 0}
          onClick={() => onSelect('yours')}
        />
        <Tile
          icon={<PlayIcon />}
          figure={running.length}
          label="En cours"
          detail={
            board.queue.running
              ? board.queue.autonomous
                ? 'file en cours · mode nuit'
                : 'file en cours'
              : 'file arrêtée'
          }
          tone="accent"
          active={running.length > 0}
          pulse={board.queue.running}
        />
        <Tile
          icon={<BulbIcon />}
          figure={board.queue.ready}
          label="Prêtes"
          detail={board.queue.next ? `Prochaine : ${board.queue.next.title}` : 'Rien de cadré'}
          tone="neutral"
          active={board.queue.ready > 0}
          title={board.queue.next?.text}
          onClick={() => onSelect('ready')}
        />
        <Tile
          icon={<CheckIcon />}
          figure={done.length}
          label="Terminées"
          detail={`${summary.integrated} ${singularForm(summary.integrated, 'intégrée', 'intégrées')}`}
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
