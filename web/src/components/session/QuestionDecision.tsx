/**
 * The pending `ask_user` question. It is a row of the decision bar so the
 * human can answer with one click or in the text box.
 */
import type { PendingQuestion } from '../../lib/fold';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { DecisionRow } from './DecisionBar';
import { markdownComponents } from './markdownComponents';
import { ReplyIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';
import { splitTitle } from '../../lib/titleLine';

export function QuestionDecision({
  question,
  onAnswer,
  disabled,
  open,
  onToggle
}: {
  question: PendingQuestion;
  onAnswer: (answer: string) => void;
  disabled: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const { title, rest } = splitTitle(question.question);
  const meta = question.type === 'choice'
    ? `${t('questionHeader')} · ${t('questionChoices', { count: question.options.length })}`
    : t('questionHeader');
  const answerButton = (
    <Button variant="primary" size="sm" onClick={onToggle}>
      {t('decisionAnswer')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<ReplyIcon className="h-3.5 w-3.5" />}
      title={title}
      meta={meta}
      primary={answerButton}
      open={open}
      onToggle={onToggle}
      footer={
        question.type === 'choice' ? (
          <>
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
                  {option.description && <span className="font-normal opacity-75"> {option.description}</span>}
                </span>
              </Button>
            ))}
          </>
        ) : question.type === 'confirm' ? (
          <>
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
          </>
        ) : (
          <span className="text-xs text-muted">{t('answerFreely')}</span>
        )
      }
    >
      {rest && (
        <div className="mb-2 min-w-0 text-sm leading-relaxed [overflow-wrap:break-word]">
          <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
            {rest}
          </ReactMarkdown>
        </div>
      )}
      {question.type !== 'text' && <div className="text-xs text-faint">{t('answerFreely')}</div>}
    </DecisionRow>
  );
}
