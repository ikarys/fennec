import { useState } from 'react';
import { useT } from '../../../i18n';
import type { RunWaiting } from '../../../lib/workflowRun';
import { StopIcon } from '../../Icons';
import { Button } from '../../ui/Button';
import { TextInput } from '../../ui/Field';

export function RunActionCard({
  waiting,
  onAnswer,
  onContinue,
  onResume,
  onStop,
  disabled
}: {
  waiting: RunWaiting;
  onAnswer: (choice: string, comment: string) => Promise<void>;
  onContinue: (comment: string) => Promise<void>;
  onResume: () => Promise<void>;
  onStop: () => Promise<void>;
  disabled: boolean;
}) {
  const t = useT();
  const [comment, setComment] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function handleAction(action: () => Promise<void>) {
    if (busy || disabled) return;
    setBusy(true);
    setError(null);
    try {
      await action();
      setComment('');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('error'));
    } finally {
      setBusy(false);
    }
  }

  const isBusy = busy || disabled;

  return (
    <div className="border-t border-border bg-panel px-6 py-2">
      <div className="mx-auto flex max-w-[960px] flex-col gap-2">

          {waiting.kind === 'human' && (
            <>
              <div className="max-h-48 overflow-y-auto text-sm leading-snug whitespace-pre-wrap [overflow-wrap:break-word]">
                {waiting.question}
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                <TextInput
                  type="text"
                  value={comment}
                  onChange={(e) => setComment(e.target.value)}
                  placeholder={t('runActionCardCommentPlaceholder')}
                  disabled={isBusy}
                  className="min-w-40 flex-1"
                />
                {waiting.choices.map((choice, index) => (
                  <Button
                    key={choice}
                    type="button"
                    variant={index === 0 ? 'primary' : 'secondary'}
                    size="sm"
                    disabled={isBusy}
                    onClick={() => handleAction(() => onAnswer(choice, comment))}
                  >
                    {choice}
                  </Button>
                ))}
                <Button
                  type="button"
                  variant="danger"
                  size="sm"
                  iconOnly
                  disabled={isBusy}
                  onClick={() => handleAction(onStop)}
                  title={t('runActionCardStopWorkflowTitle')}
                  aria-label={t('stop')}
                >
                  <StopIcon className="h-4 w-4" />
                </Button>
              </div>
            </>
          )}

          {waiting.kind === 'pause' && (
            <>
              <div className="text-sm leading-snug text-muted">
                {t('runActionCardPausedBefore', { toLabel: waiting.toLabel })}
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                <TextInput
                  type="text"
                  value={comment}
                  onChange={(e) => setComment(e.target.value)}
                  placeholder={t('runActionCardCommentPlaceholder')}
                  disabled={isBusy}
                  className="min-w-40 flex-1"
                />
                <Button
                  type="button"
                  variant="primary"
                  size="sm"
                  disabled={isBusy}
                  onClick={() => handleAction(() => onContinue(comment))}
                >
                  {t('runActionCardContinue')}
                </Button>
                <Button
                  type="button"
                  variant="danger"
                  size="sm"
                  iconOnly
                  disabled={isBusy}
                  onClick={() => handleAction(onStop)}
                  title={t('runActionCardStopWorkflowTitle')}
                  aria-label={t('stop')}
                >
                  <StopIcon className="h-4 w-4" />
                </Button>
              </div>
            </>
          )}

          {waiting.kind === 'interrupted' && (
            <>
              <div className="text-sm leading-snug text-muted">
                {t('runActionCardInterruptedQuestion')}
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                <Button
                  type="button"
                  variant="primary"
                  size="sm"
                  disabled={isBusy}
                  onClick={() => handleAction(onResume)}
                >
                  {t('runActionCardResume')}
                </Button>
                <Button
                  type="button"
                  variant="danger"
                  size="sm"
                  iconOnly
                  disabled={isBusy}
                  onClick={() => handleAction(onStop)}
                  title={t('runActionCardStopWorkflowTitle')}
                  aria-label={t('stop')}
                >
                  <StopIcon className="h-4 w-4" />
                </Button>
              </div>
            </>
          )}

          {waiting.kind === 'relaunch' && (
            <>
              <div className="text-sm leading-snug text-muted">
                {t('runActionCardRelaunchQuestion')}
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                <Button
                  type="button"
                  variant="primary"
                  size="sm"
                  disabled={isBusy}
                  onClick={() => handleAction(onResume)}
                >
                  {t('runActionCardRelaunch')}
                </Button>
              </div>
            </>
          )}

          {error && <div className="text-xs text-err">{error}</div>}
        </div>
      </div>
    );
}
