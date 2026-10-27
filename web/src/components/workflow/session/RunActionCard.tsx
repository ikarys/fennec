import { useState } from 'react';
import type { RunWaiting } from '../../../lib/workflowRun';

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
      setError(err instanceof Error ? err.message : 'Erreur');
    } finally {
      setBusy(false);
    }
  }

  const isBusy = busy || disabled;

  return (
    <div className="px-6 py-2">
      <div className="mx-auto max-w-[820px] rounded-xl border border-border bg-panel px-3 py-2">
        <div className="flex items-center gap-1.5 text-xs font-semibold">
          <span className="text-accent">⚡</span>
          <span className="text-accent-strong">Workflow</span>
        </div>

        {waiting.kind === 'human' && (
          <>
            <div className="mt-1 max-h-48 overflow-y-auto text-sm leading-snug whitespace-pre-wrap [overflow-wrap:break-word]">
              {waiting.question}
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              <input
                type="text"
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Commentaire (optionnel)…"
                disabled={isBusy}
                className="min-w-40 flex-1 rounded-md border border-border bg-transparent px-2 py-1 text-xs outline-none focus:border-accent disabled:opacity-60"
              />
              {waiting.choices.map((choice, index) => (
                <button
                  key={choice}
                  type="button"
                  disabled={isBusy}
                  onClick={() => handleAction(() => onAnswer(choice, comment))}
                  className={
                    index === 0
                      ? 'rounded-md bg-accent px-2.5 py-1 text-xs font-medium text-on-accent hover:opacity-90 disabled:opacity-60'
                      : 'rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong disabled:opacity-60'
                  }
                >
                  {choice}
                </button>
              ))}
            </div>
          </>
        )}

        {waiting.kind === 'pause' && (
          <>
            <div className="mt-1 text-sm leading-snug text-muted">
              En pause avant « {waiting.toLabel} »
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              <input
                type="text"
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Commentaire (optionnel)…"
                disabled={isBusy}
                className="min-w-40 flex-1 rounded-md border border-border bg-transparent px-2 py-1 text-xs outline-none focus:border-accent disabled:opacity-60"
              />
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(() => onContinue(comment))}
                className="rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong disabled:opacity-60"
              >
                Continuer
              </button>
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(onStop)}
                className="rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong hover:text-err disabled:opacity-60"
              >
                Arrêter
              </button>
            </div>
          </>
        )}

        {waiting.kind === 'interrupted' && (
          <>
            <div className="mt-1 text-sm leading-snug text-muted">
              Workflow interrompu
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(onResume)}
                className="rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong disabled:opacity-60"
              >
                Reprendre
              </button>
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(onStop)}
                className="rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong hover:text-err disabled:opacity-60"
              >
                Arrêter
              </button>
            </div>
          </>
        )}

        {error && <div className="mt-1.5 text-xs text-err">{error}</div>}
      </div>
    </div>
  );
}
