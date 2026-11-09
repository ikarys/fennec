/**
 * The AGENTS.md draft, a row of the decision bar: writing it to the project
 * goes through a confirmation.
 */
import { useState } from 'react';

import { writeAgentsMd } from '../../lib/api';
import { DecisionRow } from './DecisionBar';
import { FileIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { useT } from '../../i18n';

export function AgentsMdDecision({
  draft,
  sessionId,
  onWritten,
  open,
  onToggle,
}: {
  draft: { exists: boolean; written: boolean };
  sessionId: string;
  onWritten: () => void;
  open: boolean;
  onToggle: () => void;
}) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  const confirm = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await writeAgentsMd(sessionId);
      setConfirmOpen(false);
      onWritten();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
      setConfirmOpen(false);
    } finally {
      setBusy(false);
    }
  };

  const writeButton = (
    <Button variant="primary" size="sm" onClick={() => setConfirmOpen(true)} disabled={busy}>
      {t('agentsMdWrite')}
    </Button>
  );

  return (
    <>
      <DecisionRow
        icon={<FileIcon className="h-3.5 w-3.5" />}
        title={t('agentsMdWriteQuestion')}
        meta={'AGENTS.md'}
        primary={writeButton}
        open={open}
        onToggle={onToggle}
        footer={
          <>
            <span className="flex-1" />
            {writeButton}
          </>
        }
      >
        <p className="m-0 text-xs text-muted">{t('agentsMdWriteHint')}</p>
        {error ? <div className="mt-1.5 text-xs text-err">{error}</div> : null}
      </DecisionRow>
      <ConfirmDialog
        open={confirmOpen}
        title={t('agentsMdWrite')}
        body={
          draft.exists
            ? t('agentsMdConfirmBodyExists')
            : t('agentsMdConfirmBodyCreate')
        }
        confirmLabel={draft.exists ? t('agentsMdReplace') : t('writeAction')}
        variant={draft.exists ? 'danger' : undefined}
        onConfirm={confirm}
        onCancel={() => setConfirmOpen(false)}
      />
    </>
  );
}
