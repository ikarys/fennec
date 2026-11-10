/**
 * Rewrites a task in place: a draft's text, or the plan a ready task will be built from.
 * Mounted only while open, so it always starts from the current content.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { Modal } from '../../ui/Modal';
import { TextArea } from '../../ui/Field';
import { CloseIcon } from '../../ui/Icons';
import { editIdea, markIdeaReady, type IdeaBoard, type IdeaView } from '../../../lib/ideas';
import { useT } from '../../../i18n';

export function EditIdeaDialog({
  idea,
  projectId,
  field,
  run,
  onClose,
}: {
  idea: IdeaView;
  projectId: string;
  field: 'text' | 'plan';
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  onClose: () => void;
}) {
  const t = useT();
  const [value, setValue] = useState(() => (field === 'plan' ? idea.plan ?? '' : idea.text));
  const [busy, setBusy] = useState(false);

  const title = t(field === 'plan' ? 'editPlanTitle' : 'editTextTitle');
  const hint = t(field === 'plan' ? 'editPlanHint' : 'editTextHint');

  const handleSave = async () => {
    setBusy(true);
    let ok = false;
    try {
      ok = await run(() =>
        field === 'plan'
          ? markIdeaReady(projectId, idea.id, value.trim())
          : editIdea(projectId, idea.id, value.trim()),
      );
    } finally {
      setBusy(false);
    }
    if (ok) {
      onClose();
    }
  };

  return (
    <Modal open onClose={onClose} busy={busy} label={title} width="w-[min(92vw,720px)]">
      <div className="flex flex-col gap-3 p-4">
        <div className="flex items-center justify-between">
          <h2 className="m-0 text-sm font-medium">{title}</h2>
          <Button variant="ghost" size="sm" iconOnly title={t('close')} aria-label={t('close')} onClick={onClose}>
            <CloseIcon className="h-4 w-4" />
          </Button>
        </div>
        <p className="m-0 text-xs text-muted">{hint}</p>
        <TextArea
          autoFocus
          rows={field === 'plan' ? 16 : 6}
          className={field === 'plan' ? 'font-mono text-xs' : undefined}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <div className="flex items-center justify-end gap-2 border-t border-border pt-3">
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" disabled={!value.trim() || busy} onClick={() => void handleSave()}>
            {t('save')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
