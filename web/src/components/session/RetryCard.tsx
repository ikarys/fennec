/**
 * The retry action for a turn that ended in an error: re-runs the turn with
 * a system-generated continuation prompt. Rendered above the composer, like
 * the question card.
 */
import { useState } from 'react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

export function RetryCard({
  onRetry,
  disabled
}: {
  onRetry: () => Promise<void>;
  disabled: boolean;
}) {
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
      setError(err instanceof Error ? err.message : 'Erreur');
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
            <span className="text-err">Erreur</span>
          </div>
          <div className="text-sm leading-snug whitespace-pre-wrap [overflow-wrap:break-word]">
            Le tour s'est terminé en erreur : réessayer ?
          </div>
          <div className="flex gap-1.5">
            <Button
              type="button"
              variant="primary"
              size="sm"
              disabled={isBusy}
              onClick={handleRetry}
            >
              Réessayer
            </Button>
          </div>
          {error && <div className="text-xs text-err">{error}</div>}
        </div>
      </Card>
    </div>
  );
}
