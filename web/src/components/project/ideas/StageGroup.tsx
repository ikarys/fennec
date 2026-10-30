/**
 * One stage of the tasks board: a labelled group of idea rows.
 */
import { SectionLabel } from '../../ui/SectionLabel';
import { IdeaRow } from './IdeaRow';
import {
  reorderIdeas,
  type IdeaBoard,
  type IdeaView,
  type Stage,
} from '../../../lib/ideas';

const STAGE_LABELS: Record<Stage, string> = {
  yours: 'À toi',
  running: 'En cours',
  queue: 'File',
  drafts: 'Idées',
  done: 'Terminées',
};

export function StageGroup({
  stage,
  ideas,
  projectId,
  onFrame,
  onOpenSession,
  run,
  showLabel = true,
}: {
  stage: Stage;
  ideas: IdeaView[];
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  showLabel?: boolean;
}) {
  // Swap two adjacent ids and send the full new order to the server.
  const handleReorder = async (index: number, direction: -1 | 1) => {
    const ids = ideas.map((i) => i.id);
    const target = index + direction;
    if (target < 0 || target >= ids.length) return;
    [ids[index], ids[target]] = [ids[target], ids[index]];
    await run(() => reorderIdeas(projectId, ids));
  };

  return (
    <section>
      {showLabel ? <SectionLabel>{STAGE_LABELS[stage]}</SectionLabel> : null}

      <div className={showLabel ? 'mt-2 flex flex-col gap-1' : 'flex flex-col gap-1'}>
        {stage === 'queue' && ideas.length === 0 ? (
          <p className="m-0 text-sm text-muted">Aucune tâche prête : cadre une idée du backlog.</p>
        ) : null}
        {ideas.map((idea, index) => (
          <IdeaRow
            key={idea.id}
            idea={idea}
            projectId={projectId}
            onFrame={onFrame}
            onOpenSession={onOpenSession}
            run={run}
            position={stage === 'queue' ? idea.position : null}
            onMoveUp={
              stage === 'queue' && index > 0
                ? () => void handleReorder(index, -1)
                : null
            }
            onMoveDown={
              stage === 'queue' && index < ideas.length - 1
                ? () => void handleReorder(index, 1)
                : null
            }
          />
        ))}

      </div>
    </section>
  );
}
