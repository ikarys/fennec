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
    <div className="px-6 py-2">
      <div className="mx-auto max-w-[820px] rounded-xl border border-border bg-panel px-3 py-2">
        <div className="flex items-center gap-1.5 text-xs font-semibold">
          <span className="text-accent">?</span>
          <span className="text-accent-strong">Question</span>
        </div>
        <div className="mt-1 max-h-48 overflow-y-auto text-sm leading-snug whitespace-pre-wrap [overflow-wrap:break-word]">
          {question.question}
        </div>
        {question.type === 'choice' && question.options.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {question.options.map((option, index) => (
              <button
                key={option.value || index}
                type="button"
                disabled={disabled}
                onClick={() => onAnswer(option.value)}
                className="rounded-md border border-border bg-raised px-2.5 py-1 text-left text-xs text-text hover:border-border-strong disabled:opacity-60"
              >
                <span className="font-medium">{option.label}</span>
                {option.description && (
                  <span className="ml-1.5 text-muted">{option.description}</span>
                )}
              </button>
            ))}
          </div>
        )}
        {question.type === 'confirm' && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAnswer('yes')}
              className="rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong disabled:opacity-60"
            >
              Oui
            </button>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAnswer('no')}
              className="rounded-md border border-border bg-raised px-2.5 py-1 text-xs text-text hover:border-border-strong disabled:opacity-60"
            >
              Non
            </button>
          </div>
        )}
        <div className="mt-1.5 text-[11px] text-faint">
          Ou réponds librement dans la zone de saisie.
        </div>
      </div>
    </div>
  );
}
