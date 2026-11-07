// Tries a session's change where it lives, its worktree: the human checks nothing out.
import { Card } from '../ui/Card';
import { Button } from '../ui/Button';
import { Modal } from '../ui/Modal';
import { PlayIcon } from '../ui/Icons';
import { ServerList } from './ServerList';
import { useT } from '../../i18n';

interface TryDialogProps {
  sessionId: string;
  open: boolean;
  onClose: () => void;
}

export function TryDialog({ sessionId, open, onClose }: TryDialogProps) {
  const t = useT();

  return (
    <Modal
      open={open}
      onClose={onClose}
      label={t('tryChange')}
      width="w-[min(92vw,640px)]"
    >
      <Card className="overflow-hidden">
        <div className="border-b border-border bg-accent-soft px-4 py-3">
          <div className="flex items-center gap-2">
            <PlayIcon className="h-4 w-4 text-accent" />
            <h2 className="m-0 text-sm font-semibold">{t('tryChange')}</h2>
          </div>
          <p className="m-0 mt-1 text-xs text-muted">{t('tryChangeHint')}</p>
        </div>

        <div className="p-4">
          {open && <ServerList sessionId={sessionId} />}
        </div>

        <div className="flex justify-end border-t border-border px-4 py-3">
          <Button variant="secondary" size="sm" onClick={onClose}>
            {t('close')}
          </Button>
        </div>
      </Card>
    </Modal>
  );
}
