/**
 * One ready idea in the queue, as a card: the plan stands out because it is
 * what the build will run on. The card stays reorderable like a row.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { PlayIcon, EyeIcon, GripIcon } from '../../Icons';
import { startIdea, type IdeaBoard, type IdeaView } from '../../../lib/ideas';
import type { RowReorder } from './IdeaRow';

export function ReadyCard({
  idea,
  projectId,
  onOpenSession,
  run,
  position,
  reorder,
}: {
  idea: IdeaView;
  projectId: string;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  position: number | null;
  reorder: RowReorder;
}) {
  const [busy, setBusy] = useState(false);

  const handleStart = async () => {
    setBusy(true);
    try {
      await run(() => startIdea(projectId, idea.id));
    } finally {
      setBusy(false);
    }
  };

  const criteriaCount = idea.criteria
    ? idea.criteria.passed + idea.criteria.failed + idea.criteria.pending
    : null;

  return (
    <Card
      className={[
        'relative flex gap-2 p-3 pl-4',
        reorder.dragging ? 'opacity-50' : null,
        reorder.over ? 'ring-1 ring-accent' : null,
      ]
        .filter(Boolean)
        .join(' ')}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = 'move';
        // Firefox starts no drag without data.
        e.dataTransfer.setData('text/plain', idea.id);
        reorder.onDragStart();
      }}
      onDragOver={(e) => {
        e.preventDefault();
        reorder.onDragOver();
      }}
      onDrop={(e) => {
        e.preventDefault();
        reorder.onDrop();
      }}
      onDragEnd={reorder.onDragEnd}
    >
      <span className="absolute inset-y-0 left-0 w-0.5 rounded-l-xl bg-accent" aria-hidden />
      <div className="flex shrink-0 items-center gap-1 pt-0.5">
        {position != null ? (
          <span className="w-5 text-right font-mono text-xs text-muted">{position}</span>
        ) : null}
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title="Glisser pour réordonner (↑/↓ au clavier)"
          aria-label="Réordonner"
          className="cursor-grab"
          onKeyDown={(e) => {
            if (e.key === 'ArrowUp') {
              e.preventDefault();
              reorder.onKeyMove(-1);
            } else if (e.key === 'ArrowDown') {
              e.preventDefault();
              reorder.onKeyMove(1);
            }
          }}
        >
          <GripIcon className="h-4 w-4" />
        </Button>
      </div>

      <div className="min-w-0 grow">
        <p className="m-0 text-sm font-medium text-text">{idea.title}</p>
        {idea.plan ? (
          <div className="mt-2 rounded-md border border-border bg-bg p-2.5">
            <p className="m-0 text-xs text-faint">Plan (produit par le modèle, indicatif)</p>
            <p className="m-0 mt-1 whitespace-pre-line text-xs leading-relaxed text-muted">
              {idea.plan}
            </p>
          </div>
        ) : null}
        {idea.halted ? <p className="m-0 mt-1 text-xs text-muted">{idea.halted}</p> : null}
        <div className="mt-2 flex items-center gap-2 text-xs">
          {criteriaCount != null ? (
            <span className="rounded bg-raised px-1.5 py-0.5 text-muted">{criteriaCount} critères</span>
          ) : null}
          {idea.decisions.length > 0 ? (
            <span className="rounded bg-raised px-1.5 py-0.5 text-muted">
              {idea.decisions.length} décision{idea.decisions.length > 1 ? 's' : ''}
            </span>
          ) : null}
        </div>
      </div>

      <div className="flex shrink-0 flex-col items-end gap-1.5">
        {idea.sessionId ? (
          <Button
            variant="ghost"
            size="sm"
            title="Ouvrir la session de cadrage"
            onClick={() => onOpenSession(idea.sessionId as string)}
          >
            <EyeIcon className="h-4 w-4" />
            Voir le plan
          </Button>
        ) : null}
        <Button
          variant="soft"
          size="sm"
          title="Construire cette idée maintenant"
          disabled={busy}
          onClick={() => void handleStart()}
        >
          <PlayIcon className="h-4 w-4" />
          Lancer
        </Button>
      </div>
    </Card>
  );
}
