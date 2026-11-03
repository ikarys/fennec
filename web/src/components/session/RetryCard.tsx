/**
 * The retry action for a turn that ended in an error: re-runs the turn with
 * a system-generated continuation prompt. Rendered above the composer, like
 * the question card.
 */
import { useState } from 'react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { useT } from '../../i18n';

export function RetryCard({
  onRetry,
  disabled
}: {
  onRetry: () => Promise<void>;
  disabled: boolean;
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

  return (
    <div className="px-6 py-2">
      <Card className="mx-auto max-w-[820px] px-3 py-2">
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1.5 text-xs font-semibold">
            <span className="text-err">!</span>
            <span className="text-err">{t('error')}</span>
          </div>
          <div className="text-sm leading-snug whitespace-pre-wrap [overflow-wrap:break-word]">
            {t('retryPrompt')}
          </div>
          <div className="flex gap-1.5">
            <Button
              type="button"
              variant="primary"
              size="sm"
              disabled={isBusy}
              onClick={handleRetry}
            >
              {t('retry')}
            </Button>
          </div>
          {error && <div className="text-xs text-err">{error}</div>}
        </div>
      </Card>
    </div>
  );
}
