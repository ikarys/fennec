/**
 * One ready idea in the queue, as a card. It keeps what a row shows: status,
 * attachments, run meta; the plan stays in its framing session ("Voir le
 * plan"), too long for a card. The card stays reorderable like a row.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { PlayIcon, EyeIcon, GripIcon } from '../../Icons';
import { startIdea, type IdeaBoard, type IdeaView } from '../../../lib/ideas';
import type { RowReorder } from './IdeaRow';
import { IdeaMenu } from './IdeaMenu';
import { IdeaMeta } from './IdeaMeta';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaAttachments } from './IdeaAttachments';

export function ReadyCard({
  idea,
  projectId,
  onFrame,
  onOpenSession,
  run,
  caps,
  position,
  reorder,
}: {
  idea: IdeaView;
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  caps: IdeaBoard['queue']['caps'];
  position: number | null;
  reorder: RowReorder | null;
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

  return (
    <Card
      className={[
        'relative flex gap-2 p-3 pl-4',
        reorder?.dragging ? 'opacity-50' : null,
        reorder?.over ? 'ring-1 ring-accent' : null,
      ]
        .filter(Boolean)
        .join(' ')}
      draggable={reorder ? true : undefined}
      onDragStart={
        reorder
          ? (e) => {
              e.dataTransfer.effectAllowed = 'move';
              // Firefox starts no drag without data.
              e.dataTransfer.setData('text/plain', idea.id);
              reorder.onDragStart();
            }
          : undefined
      }
      onDragOver={
        reorder
          ? (e) => {
              e.preventDefault();
              reorder.onDragOver();
            }
          : undefined
      }
      onDrop={
        reorder
          ? (e) => {
              e.preventDefault();
              reorder.onDrop();
            }
          : undefined
      }
      onDragEnd={reorder ? reorder.onDragEnd : undefined}
    >
      <span className="absolute inset-y-0 left-0 w-0.5 rounded-l-xl bg-accent" aria-hidden />
      <div className="flex shrink-0 items-center gap-1 pt-0.5">
        {position != null ? (
          <span className="w-5 text-right font-mono text-xs text-muted">{position}</span>
        ) : null}
        {reorder ? (
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
        ) : null}
      </div>

      <div className="min-w-0 grow">
        <div className="flex items-center gap-2">
          <p className="m-0 truncate text-sm font-medium text-text" title={idea.title}>
            {idea.title}
          </p>
          <IdeaStatusBadge status={idea.status} />
        </div>
        <IdeaAttachments attachments={idea.attachments} />
        {idea.halted ? <p className="m-0 mt-1 text-xs text-muted">{idea.halted}</p> : null}
        <div className="mt-1.5 flex flex-wrap items-center gap-1.5 text-xs">
          <IdeaMeta idea={idea} caps={caps} />
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
        <IdeaMenu idea={idea} projectId={projectId} onFrame={onFrame} run={run} />
      </div>
    </Card>
  );
}
