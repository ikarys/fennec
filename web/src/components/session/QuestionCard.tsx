/**
 * The pending `ask_user` question. It is rendered above the composer so the
 * human can answer with one click or in the text box.
 */
import type { PendingQuestion } from '../../lib/fold';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { markdownComponents } from './markdownComponents';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { useT } from '../../i18n';

export function QuestionCard({
  question,
  onAnswer,
  disabled
}: {
  question: PendingQuestion;
  onAnswer: (answer: string) => void;
  disabled: boolean;
}) {
  const t = useT();
  return (
    <div className="px-6 py-2">
      <Card className="mx-auto max-w-[820px] px-3 py-2">
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1.5 text-xs font-semibold">
            <span className="text-accent">?</span>
            <span className="text-accent-strong">{t('questionHeader')}</span>
          </div>
          <div className="min-w-0 max-h-96 overflow-y-auto text-sm leading-relaxed [overflow-wrap:break-word]">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
              {question.question}
            </ReactMarkdown>
          </div>
          {question.type === 'choice' && question.options.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {question.options.map((option, index) => (
                <Button
                  key={option.value || index}
                  type="button"
                  variant={option.tone === 'danger' ? 'danger' : index === 0 ? 'primary' : 'secondary'}
                  size="sm"
                  multiline
                  disabled={disabled}
                  onClick={() => onAnswer(option.value)}
                  className="max-w-full"
                >
                  <span>
                    <span className="font-medium">{option.label}</span>
                    {/* Inherits the variant's text colour, dimmed: readable on the accent too. */}
                    {option.description && <span className="font-normal opacity-75"> {option.description}</span>}
                  </span>
                </Button>
              ))}
            </div>
          )}
          {question.type === 'confirm' && (
            <div className="flex flex-wrap gap-1.5">
              <Button
                type="button"
                variant="primary"
                size="sm"
                disabled={disabled}
                onClick={() => onAnswer('yes')}
              >
                {t('yes')}
              </Button>
              <Button
                type="button"
                variant="danger"
                size="sm"
                disabled={disabled}
                onClick={() => onAnswer('no')}
              >
                {t('no')}
              </Button>
            </div>
          )}
          <div className="text-xs text-faint">{t('answerFreely')}</div>
        </div>
      </Card>
    </div>
  );
}
