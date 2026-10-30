/**
 * The add-idea field, opened on demand above the tasks; stays open to chain ideas.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { TextInput } from '../../ui/Field';
import { PlusIcon } from '../../Icons';
import { addIdea, type IdeaBoard } from '../../../lib/ideas';

export function AddIdeaField({
  projectId,
  run,
  onClose,
}: {
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  onClose: () => void;
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

  return (
    <div className="relative">
      <TextInput
        autoFocus
        className="w-full pr-10"
        placeholder="Une idée à garder pour plus tard… (Échap pour fermer)"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          if (!text.trim()) onClose();
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void handleAdd();
          } else if (e.key === 'Escape') {
            onClose();
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
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => void handleAdd()}
      >
        <PlusIcon className="h-4 w-4" />
      </Button>
    </div>
  );
}
