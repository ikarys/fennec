/**
 * The one-click write of the agent's AGENTS.md draft into the project root.
 *
 * Agents read AGENTS.md but never write it: the human confirms and fennec
 * writes the file in the human's tree, without committing it.
 */
import { useState } from 'react';

import { useT } from '../../i18n';
import { writeAgentsMd } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { ConfirmDialog } from '../ui/ConfirmDialog';

export function AgentsMdCard({
  draft,
  sessionId,
  onWritten,
}: {
  draft: { exists: boolean; written: boolean };
  sessionId: string | null;
  onWritten: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  if (!sessionId) return null;

  if (draft.written) {
    return (
      <Card className="p-4">
        <p className="m-0 text-sm text-muted">
          {t('agentsMdWritten')}
        </p>
      </Card>
    );
  }

  const confirm = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await writeAgentsMd(sessionId);
      setOpen(false);
      onWritten();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
      setOpen(false);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-1">
        <p className="m-0 text-sm font-medium text-text">{t('agentsMdWriteQuestion')}</p>
        <p className="m-0 text-xs text-muted">
          {t('agentsMdWriteHint')}
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Button variant="primary" size="md" onClick={() => setOpen(true)} disabled={busy}>
          {t('agentsMdWrite')}
        </Button>
      </div>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      <ConfirmDialog
        open={open}
        title={t('agentsMdWrite')}
        body={
          draft.exists
            ? t('agentsMdConfirmBodyExists')
            : t('agentsMdConfirmBodyCreate')
        }
        confirmLabel={draft.exists ? t('agentsMdReplace') : t('writeAction')}
        variant={draft.exists ? 'danger' : undefined}
        onConfirm={confirm}
        onCancel={() => setOpen(false)}
      />
    </Card>
  );
}
