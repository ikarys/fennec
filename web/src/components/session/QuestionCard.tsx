/**
 * The pending `ask_user` question. It is rendered above the composer so the
 * human can answer with one click or in the text box.
 */
import type { PendingQuestion } from '../../lib/fold';

export function QuestionCard({
  question,
  onAnswer,
  disabled
}: {
  question: PendingQuestion;
  onAnswer: (answer: string) => void;
  disabled: boolean;
}) {
  return (
    <div className="px-6 py-3">
      <div className="mx-auto max-w-[820px] rounded-[14px] border border-border bg-panel">
        <div className="flex items-center gap-2 px-3.5 pt-3 text-sm font-semibold">
          <span className="text-accent">?</span>
          <span className="text-accent-strong">Question</span>
        </div>
        <div className="px-3.5 pb-3 pt-2 text-sm leading-relaxed whitespace-pre-wrap [overflow-wrap:break-word]">
          {question.question}
        </div>
        {question.type === 'choice' && question.options.length > 0 && (
          <div className="flex flex-col gap-2 px-3.5 pb-3">
            {question.options.map((option, index) => (
              <button
                key={option.value || index}
                type="button"
                disabled={disabled}
                onClick={() => onAnswer(option.value)}
                className="rounded-lg border border-border bg-raised px-3 py-2 text-left text-sm text-text disabled:opacity-60"
              >
                <span className="block font-medium">{option.label}</span>
                {option.description && (
                  <span className="mt-0.5 block text-xs text-muted">{option.description}</span>
                )}
              </button>
            ))}
          </div>
        )}
        {question.type === 'confirm' && (
          <div className="flex gap-2 px-3.5 pb-3">
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAnswer('yes')}
              className="rounded-lg border border-border bg-raised px-3 py-2 text-sm text-text disabled:opacity-60"
            >
              Oui
            </button>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAnswer('no')}
              className="rounded-lg border border-border bg-raised px-3 py-2 text-sm text-text disabled:opacity-60"
            >
              Non
            </button>
          </div>
        )}
        <div className="border-t border-border px-3.5 py-2 text-xs text-muted">
          Ou réponds librement dans la zone de saisie.
        </div>
      </div>
    </div>
  );
}
