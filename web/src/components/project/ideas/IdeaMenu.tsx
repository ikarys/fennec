/**
 * The shared ⋯ menu for an idea: reframe, start, restart, complete, drop.
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
  const [pendingDrop, setPendingDrop] = useState(false);
  if (idea.menu.length === 0) {
    return null;
  }

  // The row's busy state guards its primary action; the menu's items each run once.
  const handleFrame = () => void onFrame(idea);
  const handleStart = () => void run(() => startIdea(projectId, idea.id));
  const handleComplete = () => void run(() => completeIdea(projectId, idea.id));
  const handleDrop = () => {
    setPendingDrop(false);
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
                <MenuItem key={action} tone="danger" onSelect={() => setPendingDrop(true)}>
                  Abandonner
                </MenuItem>
              );
          }
        })}
      </Menu>
      <ConfirmDialog
        open={pendingDrop}
        title="Abandonner cette idée ?"
        body="L'idée sort de la file. Cette action est définitive."
        confirmLabel="Abandonner"
        onConfirm={handleDrop}
        onCancel={() => setPendingDrop(false)}
      />
    </>
  );
}
