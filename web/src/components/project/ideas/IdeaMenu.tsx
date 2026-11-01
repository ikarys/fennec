/**
 * The shared ⋯ menu for an idea: reframe, start, restart, complete, drop, remove.
 */
import { useState } from 'react';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { Menu, MenuItem } from '../../ui/Menu';
import {
  completeIdea,
  dropIdea,
  startIdea,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';

export function IdeaMenu({
  idea,
  projectId,
  onFrame,
  run,
}: {
  idea: IdeaView;
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
}) {
  // Both drop and remove end in `idea.dropped`; only the wording differs.
  const [pendingDrop, setPendingDrop] = useState<'drop' | 'remove' | null>(null);
  if (idea.menu.length === 0) {
    return null;
  }

  // The row's busy state guards its primary action; the menu's items each run once.
  const handleFrame = () => void onFrame(idea);
  const handleStart = () => void run(() => startIdea(projectId, idea.id));
  const handleComplete = () => void run(() => completeIdea(projectId, idea.id));
  const handleDrop = () => {
    setPendingDrop(null);
    void run(() => dropIdea(projectId, idea.id));
  };

  return (
    <>
      <Menu label="Actions">
        {idea.menu.map((action) => {
          switch (action) {
            case 'reframe':
              return (
                <MenuItem key={action} onSelect={handleFrame}>
                  Recadrer
                </MenuItem>
              );
            case 'startUnframed':
              return (
                <MenuItem key={action} onSelect={handleStart}>
                  Lancer sans cadrage
                </MenuItem>
              );
            case 'restart':
              return (
                <MenuItem key={action} onSelect={handleStart}>
                  Relancer
                </MenuItem>
              );
            case 'complete':
              return (
                <MenuItem key={action} onSelect={handleComplete}>
                  Marquer comme faite
                </MenuItem>
              );
            case 'drop':
              return (
                <MenuItem key={action} tone="danger" onSelect={() => setPendingDrop('drop')}>
                  Abandonner
                </MenuItem>
              );
            case 'remove':
              return (
                <MenuItem key={action} tone="danger" onSelect={() => setPendingDrop('remove')}>
                  Supprimer
                </MenuItem>
              );
          }
        })}
      </Menu>
      <ConfirmDialog
        open={pendingDrop !== null}
        title={pendingDrop === 'remove' ? 'Supprimer cette tâche ?' : 'Abandonner cette idée ?'}
        body={
          pendingDrop === 'remove'
            ? 'La tâche sort du tableau ; sa session et son historique restent. Cette action est définitive.'
            : "L'idée sort de la file. Cette action est définitive."
        }
        confirmLabel={pendingDrop === 'remove' ? 'Supprimer' : 'Abandonner'}
        onConfirm={handleDrop}
        onCancel={() => setPendingDrop(null)}
      />
    </>
  );
}
