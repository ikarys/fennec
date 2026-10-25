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
    <div className="px-6 py-3">
      <div className="mx-auto max-w-[820px] rounded-[14px] border border-border bg-panel">
        <div className="flex items-center gap-2 px-3.5 pt-3 text-sm font-semibold">
          <span className="text-accent">⚡</span>
          <span className="text-accent-strong">Workflow</span>
        </div>

        {waiting.kind === 'human' && (
          <>
            <div className="px-3.5 pb-2 pt-2 text-sm leading-relaxed whitespace-pre-wrap [overflow-wrap:break-word]">
              {waiting.question}
            </div>
            <div className="flex flex-col gap-2 px-3.5 pb-3">
              {waiting.choices.map((choice) => (
                <button
                  key={choice}
                  type="button"
                  disabled={isBusy}
                  onClick={() => handleAction(() => onAnswer(choice, comment))}
                  className="rounded-lg border border-border bg-raised px-3 py-2 text-left text-sm text-text disabled:opacity-60"
                >
                  {choice}
                </button>
              ))}
            </div>
            <div className="border-t border-border px-3.5 py-2">
              <textarea
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Commentaire (optionnel)…"
                rows={2}
                disabled={isBusy}
                className="w-full resize-none rounded-md border border-border bg-transparent p-2 text-sm outline-none focus:border-accent disabled:opacity-60"
              />
            </div>
          </>
        )}

        {waiting.kind === 'pause' && (
          <>
            <div className="px-3.5 pb-2 pt-2 text-sm text-muted">
              En pause avant « {waiting.toLabel} »
            </div>
            <div className="flex gap-2 px-3.5 pb-3">
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(() => onContinue(comment))}
                className="rounded-lg border border-border bg-raised px-3 py-2 text-sm text-text disabled:opacity-60"
              >
                Continuer
              </button>
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(onStop)}
                className="rounded-lg border border-border bg-raised px-3 py-2 text-sm text-text hover:text-err disabled:opacity-60"
              >
                Arrêter
              </button>
            </div>
            <div className="border-t border-border px-3.5 py-2">
              <textarea
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Commentaire (optionnel)…"
                rows={2}
                disabled={isBusy}
                className="w-full resize-none rounded-md border border-border bg-transparent p-2 text-sm outline-none focus:border-accent disabled:opacity-60"
              />
            </div>
          </>
        )}

        {waiting.kind === 'interrupted' && (
          <>
            <div className="px-3.5 pb-2 pt-2 text-sm text-muted">
              Workflow interrompu
            </div>
            <div className="flex gap-2 px-3.5 pb-3">
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(onResume)}
                className="rounded-lg border border-border bg-raised px-3 py-2 text-sm text-text disabled:opacity-60"
              >
                Reprendre
              </button>
              <button
                type="button"
                disabled={isBusy}
                onClick={() => handleAction(onStop)}
                className="rounded-lg border border-border bg-raised px-3 py-2 text-sm text-text hover:text-err disabled:opacity-60"
              >
                Arrêter
              </button>
            </div>
          </>
        )}

        {error && <div className="border-t border-border px-3.5 py-2 text-xs text-err">{error}</div>}
      </div>
    </div>
  );
}
