/**
 * One idea in the board: text, status, meta, one primary action, and a ⋯ menu.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { Menu, MenuItem } from '../../ui/Menu';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaMeta } from './IdeaMeta';
import {
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
  open: 'Ouvrir',
  answer: 'Répondre',
  review: 'Relire',
};

export function IdeaRow({
  idea,
  projectId,
  onFrame,
  onOpenSession,
  run,
  position,
  onMoveUp,
  onMoveDown,
}: {
  idea: IdeaView;
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  position: number | null;
  onMoveUp: (() => void) | null;
  onMoveDown: (() => void) | null;
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
    switch (idea.action) {
      case 'frame':
        return (
          <Button variant="secondary" size="sm" disabled={busy} onClick={() => void handleFrame()}>
            {ACTION_LABELS.frame}
          </Button>
        );
      case 'resumeFraming':
      case 'open':
      case 'answer':
      case 'review':
        if (!idea.sessionId) return null;
        return (
          <Button
            variant="secondary"
            size="sm"
            disabled={busy}
            onClick={() => onOpenSession(idea.sessionId as string)}
          >
            {ACTION_LABELS[idea.action]}
          </Button>
        );
      case 'start':
        return (
          <Button variant="secondary" size="sm" disabled={busy} onClick={() => void handleStart()}>
            {ACTION_LABELS.start}
          </Button>
        );
      default:
        return null;
    }
  };

  return (
    <div className="relative flex items-start gap-2 px-2 py-2">
      {position != null ? (
        <span className="w-5 shrink-0 text-right font-mono text-xs text-muted">
          {position}
        </span>
      ) : null}

      {onMoveUp ? (
        <Button variant="ghost" size="sm" iconOnly aria-label="Monter" onClick={onMoveUp}>
          ↑
        </Button>
      ) : null}
      {onMoveDown ? (
        <Button variant="ghost" size="sm" iconOnly aria-label="Descendre" onClick={onMoveDown}>
          ↓
        </Button>
      ) : null}

      <div className="min-w-0 grow">
        <p className="m-0 text-sm text-text line-clamp-2" title={idea.text}>
          {idea.text}
        </p>
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
