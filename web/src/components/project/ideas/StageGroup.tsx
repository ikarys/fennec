/**
 * One stage of the tasks board: a labelled group of idea rows.
 */
import { useState } from 'react';
import { SectionLabel } from '../../ui/SectionLabel';
import { Button } from '../../ui/Button';
import { TextInput } from '../../ui/Field';
import { PlusIcon } from '../../Icons';
import { IdeaRow } from './IdeaRow';
import {
  addIdea,
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
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);

  const handleAdd = async () => {
    const value = text.trim();
    if (!value || busy) return;
    setBusy(true);
    try {
      const ok = await run(() => addIdea(projectId, value));
      if (ok) setText('');
    } finally {
      setBusy(false);
    }
  };

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

        {stage === 'drafts' ? (
          <div className="relative mt-2">
            <TextInput
              className="w-full pr-10"
              placeholder="Une idée à garder pour plus tard…"
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void handleAdd();
                }
              }}
            />
            <Button
              variant="ghost"
              iconOnly
              className="absolute right-1 top-1/2 -translate-y-1/2"
              title="Ajouter l'idée au backlog (Entrée)"
              aria-label="Ajouter l'idée au backlog"
              disabled={!text.trim() || busy}
              onClick={() => void handleAdd()}
            >
              <PlusIcon className="h-4 w-4" />
            </Button>
          </div>
        ) : null}
      </div>
    </section>
  );
}
