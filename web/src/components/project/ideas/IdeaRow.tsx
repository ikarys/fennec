/**
 * One idea in the board: text, status, meta, one primary action, and a ⋯ menu.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import {
  EditIcon,
  CheckIcon,
  EyeIcon,
  FileIcon,
  GripIcon,
  PlayIcon,
  ReplyIcon,
} from '../../Icons';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { Menu, MenuItem } from '../../ui/Menu';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaMeta } from './IdeaMeta';
import {
  completeIdea,
  dropIdea,
  startIdea,
  type IdeaAction,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';

const ACTION_LABELS: Record<IdeaAction, string> = {
  frame: 'Cadrer',
  resumeFraming: 'Reprendre le cadrage',
  start: 'Lancer',
  open: 'Voir la session',
  answer: 'Répondre',
  review: 'Relire',
};

const ACTION_ICONS: Record<IdeaAction, typeof EditIcon> = {
  frame: EditIcon,
  resumeFraming: EditIcon,
  start: PlayIcon,
  open: EyeIcon,
  answer: ReplyIcon,
  review: CheckIcon,
};

export type RowReorder = {
  onDragStart: () => void;
  onDragOver: () => void;
  onDrop: () => void;
  onDragEnd: () => void;
  onKeyMove: (direction: -1 | 1) => void;
  dragging: boolean;
  over: boolean;
};

export function IdeaRow({
  idea,
  projectId,
  onFrame,
  onOpenSession,
  run,
  position,
  reorder,
}: {
  idea: IdeaView;
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  position: number | null;
  reorder: RowReorder | null;
}) {
  const [pendingDrop, setPendingDrop] = useState(false);
  const [busy, setBusy] = useState(false);

  const handleStart = async () => {
    setBusy(true);
    try {
      await run(() => startIdea(projectId, idea.id));
    } finally {
      setBusy(false);
    }
  };

  const handleDrop = async () => {
    setPendingDrop(false);
    setBusy(true);
    try {
      await run(() => dropIdea(projectId, idea.id));
    } finally {
      setBusy(false);
    }
  };

  const handleComplete = async () => {
    setBusy(true);
    try {
      await run(() => completeIdea(projectId, idea.id));
    } finally {
      setBusy(false);
    }
  };

  const handleFrame = async () => {
    setBusy(true);
    try {
      await onFrame(idea);
    } finally {
      setBusy(false);
    }
  };

  // The one primary action the server chose for this idea.
  const renderAction = () => {
    const onClick =
      idea.action === 'frame'
        ? () => void handleFrame()
        : idea.action === 'start'
          ? () => void handleStart()
          : idea.sessionId
            ? () => onOpenSession(idea.sessionId as string)
            : null;
    if (!onClick || !idea.action) return null;
    const Icon = ACTION_ICONS[idea.action];
    return (
      <Button
        variant="soft"
        size="sm"
        iconOnly
        disabled={busy}
        title={ACTION_LABELS[idea.action]}
        aria-label={ACTION_LABELS[idea.action]}
        onClick={onClick}
      >
        <Icon className="h-4 w-4" />
      </Button>
    );
  };

  return (
    <div
      className={[
        'relative flex items-start gap-2 px-2 py-2 hover:bg-raised',
        reorder?.dragging ? 'opacity-50' : null,
        reorder?.over ? 'rounded-md ring-1 ring-accent' : null,
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
      {position != null ? (
        <span className="w-5 shrink-0 text-right font-mono text-xs text-muted">
          {position}
        </span>
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

      <div className="min-w-0 grow">
        <p className="m-0 text-sm text-text line-clamp-2" title={idea.text}>
          {idea.text}
        </p>
        {idea.attachments.length > 0 ? (
          <div className="mt-1.5 flex flex-wrap gap-1">
            {idea.attachments.map((attachment, i) =>
              attachment.mimeType.startsWith('image/') ? (
                <img
                  key={i}
                  src={attachment.data}
                  alt={attachment.filename}
                  title={attachment.filename}
                  className="h-9 w-9 rounded-md border border-border object-cover"
                />
              ) : (
                <span
                  key={i}
                  title={attachment.filename}
                  className="flex items-center gap-1 rounded-md border border-border bg-bg px-1.5 py-0.5 text-[11px] text-muted"
                >
                  <FileIcon className="h-3 w-3" />
                  <span className="max-w-[120px] truncate">{attachment.filename}</span>
                </span>
              ),
            )}
          </div>
        ) : null}
        {idea.halted ? (
          <p className="mt-0.5 text-xs text-muted">{idea.halted}</p>
        ) : null}
        <div className="mt-1 flex flex-wrap items-center gap-2 text-xs">
          <IdeaStatusBadge status={idea.status} />
          <IdeaMeta idea={idea} />
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1">
        {renderAction()}
        {idea.menu.length > 0 ? (
          <Menu label="Actions">
            {idea.menu.map((action) => {
              switch (action) {
                case 'reframe':
                  return (
                    <MenuItem key={action} onSelect={() => void handleFrame()}>
                      Recadrer
                    </MenuItem>
                  );
                case 'startUnframed':
                  return (
                    <MenuItem key={action} onSelect={() => void handleStart()}>
                      Lancer sans cadrage
                    </MenuItem>
                  );
                case 'restart':
                  return (
                    <MenuItem key={action} onSelect={() => void handleStart()}>
                      Relancer
                    </MenuItem>
                  );
                case 'complete':
                  return (
                    <MenuItem key={action} onSelect={() => void handleComplete()}>
                      Marquer comme faite
                    </MenuItem>
                  );
                case 'drop':
                  return (
                    <MenuItem key={action} tone="danger" onSelect={() => setPendingDrop(true)}>
                      Abandonner
                    </MenuItem>
                  );
              }
            })}
          </Menu>
        ) : null}
      </div>

      <ConfirmDialog
        open={pendingDrop}
        title="Abandonner cette idée ?"
        body="L'idée sort de la file. Cette action est définitive."
        confirmLabel="Abandonner"
        onConfirm={() => void handleDrop()}
        onCancel={() => setPendingDrop(false)}
      />
    </div>
  );
}
