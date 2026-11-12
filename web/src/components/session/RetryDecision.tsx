/**
 * A row of the decision bar for retrying a turn that ended in an error.
 */
import { useState } from 'react';
import { DecisionRow } from './DecisionBar';
import { AlertIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';

export function RetryDecision({
  onRetry,
  disabled,
  open,
  onToggle
}: {
  onRetry: () => Promise<void>;
  disabled: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const isBusy = busy || disabled;

  async function handleRetry() {
    if (isBusy) return;
    setBusy(true);
    setError(null);
    try {
      await onRetry();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('error'));
    } finally {
      setBusy(false);
    }
  }

  const retryButton = (
    <Button
      type="button"
      variant="primary"
      size="sm"
      disabled={isBusy}
      onClick={handleRetry}
    >
      {t('retry')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<AlertIcon className="h-3.5 w-3.5" />}
      title={t('retryPrompt')}
      meta={t('error')}
      primary={retryButton}
      open={open}
      onToggle={onToggle}
      danger
      footer={
        <>
          <span className="flex-1" />
          {retryButton}
        </>
      }
    >
      {error && <div className="text-xs text-err">{error}</div>}
    </DecisionRow>
  );
}
