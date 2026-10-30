/**
 * One stage of the ideas board: a labelled group of idea rows.
 */
import { useState } from 'react';
import { SectionLabel } from '../../ui/SectionLabel';
import { TextArea } from '../../ui/Field';
import { IdeaRow } from './IdeaRow';
import {
  addIdea,
  reorderIdeas,
  type IdeaBoard,
  type IdeaView,
  type Stage,
} from '../../../lib/ideas';

const STAGE_LABELS: Record<Stage, string> = {
  yours: 'À faire par toi',
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
}: {
  stage: Stage;
  ideas: IdeaView[];
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
}) {
  // "done" starts collapsed; the others are open.
  const [collapsed, setCollapsed] = useState(stage === 'done');
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
      {stage === 'done' ? (
        <button
          type="button"
          onClick={() => setCollapsed(!collapsed)}
          className="flex w-full items-center gap-2 text-left"
          aria-expanded={!collapsed}
        >
          <SectionLabel>{STAGE_LABELS[stage]}</SectionLabel>
          <span className="text-xs text-muted">({ideas.length})</span>
        </button>
      ) : (
        <SectionLabel>{STAGE_LABELS[stage]}</SectionLabel>
      )}

      {!collapsed ? (
        <div className="mt-2 flex flex-col gap-1">
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
            <div className="mt-2">
              <TextArea
                rows={1}
                className="w-full"
                placeholder="Une idée à garder pour plus tard…"
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    void handleAdd();
                  }
                }}
              />
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
