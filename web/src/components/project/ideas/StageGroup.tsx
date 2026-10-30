/**
 * One stage of the tasks board: a labelled group of idea rows.
 */
import { useState } from 'react';
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
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  const move = async (from: number, to: number) => {
    if (
      from === to ||
      from < 0 ||
      from >= ideas.length ||
      to < 0 ||
      to >= ideas.length
    ) {
      return;
    }
    const ids = ideas.map((i) => i.id);
    const [item] = ids.splice(from, 1);
    ids.splice(to, 0, item);
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
            reorder={
              stage === 'queue' && ideas.length > 1
                ? {
                    onDragStart: () => setDragFrom(index),
                    onDragOver: () => setDragOver(index),
                    onDrop: () => {
                      if (dragFrom != null) void move(dragFrom, index);
                      setDragFrom(null);
                      setDragOver(null);
                    },
                    onDragEnd: () => {
                      setDragFrom(null);
                      setDragOver(null);
                    },
                    onKeyMove: (d: -1 | 1) => void move(index, index + d),
                    dragging: dragFrom === index,
                    over:
                      dragOver === index && dragFrom !== null && dragFrom !== index,
                  }
                : null
            }
          />
        ))}

      </div>
    </section>
  );
}
