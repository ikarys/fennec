/**
 * The shared ⋯ menu for an idea: edit, reframe, start, restart, complete, drop, remove.
 */
import { useState } from 'react';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { EditIdeaDialog } from './EditIdeaDialog';
import { Menu, MenuItem } from '../../ui/Menu';
import {
  completeIdea,
  dropIdea,
  startIdea,
  type IdeaBoard,
  type IdeaView,
} from '../../../lib/ideas';
import { useT } from '../../../i18n';

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
  const [editing, setEditing] = useState<'plan' | null>(null);
  const t = useT();
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
      <Menu label={t('menuActions')}>
        {idea.menu.map((action) => {
          switch (action) {
            case 'editPlan':
              return (
                <MenuItem key={action} onSelect={() => setEditing('plan')}>
                  {t('menuEditPlan')}
                </MenuItem>
              );
            case 'reframe':
              return (
                <MenuItem key={action} onSelect={handleFrame}>
                  {t('menuReframe')}
                </MenuItem>
              );
            case 'startUnframed':
              return (
                <MenuItem key={action} onSelect={handleStart}>
                  {t('menuStartUnframed')}
                </MenuItem>
              );
            case 'restart':
              return (
                <MenuItem key={action} onSelect={handleStart}>
                  {t('menuRestart')}
                </MenuItem>
              );
            case 'complete':
              return (
                <MenuItem key={action} onSelect={handleComplete}>
                  {t('menuComplete')}
                </MenuItem>
              );
            case 'drop':
              return (
                <MenuItem key={action} tone="danger" onSelect={() => setPendingDrop('drop')}>
                  {t('menuDrop')}
                </MenuItem>
              );
            case 'remove':
              return (
                <MenuItem key={action} tone="danger" onSelect={() => setPendingDrop('remove')}>
                  {t('menuRemove')}
                </MenuItem>
              );
          }
        })}
      </Menu>
      <ConfirmDialog
        open={pendingDrop !== null}
        title={pendingDrop === 'remove' ? t('confirmRemoveTask') : t('confirmDropIdea')}
        body={pendingDrop === 'remove' ? t('confirmRemoveBody') : t('confirmDropBody')}
        confirmLabel={pendingDrop === 'remove' ? t('menuRemove') : t('menuDrop')}
        onConfirm={handleDrop}
        onCancel={() => setPendingDrop(null)}
      />
      {editing ? (
        <EditIdeaDialog
          idea={idea}
          projectId={projectId}
          field="plan"
          run={run}
          onClose={() => setEditing(null)}
        />
      ) : null}
    </>
  );
}
