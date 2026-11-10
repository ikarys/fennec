import { useState, type FormEvent, type KeyboardEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { TextArea } from '../ui/Field';
import { BulbIcon, ReplyIcon, SendIcon } from '../ui/Icons';
import { Section } from '../ui/Section';
import { useT } from '../../i18n';

export function QuestionForm({
  busy,
  onAsk,
  onUnderstand,
}: {
  busy: boolean;
  onAsk: (question: string) => void;
  onUnderstand: (topic: string) => void;
}) {
  const [question, setQuestion] = useState('');
  const trimmed = question.trim();
  const t = useT();

  const send = (callback: (text: string) => void) => {
    if (busy || !trimmed) return;
    callback(trimmed);
    setQuestion('');
  };

  const submit = () => {
    send(onAsk);
  };

  const onSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    submit();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      submit();
    }
  };

  return (
    <Section
      Icon={ReplyIcon}
      title={t('askQuestion')}
      actions={
        <Badge tone="info" appearance="pill" title={t('indicativeSourced')}>
          {t('indicative')}
        </Badge>
      }
      flush
    >
      <form onSubmit={onSubmit} className="flex flex-col gap-3 p-4">
        <TextArea
          id="project-question"
          rows={3}
          value={question}
          placeholder={t('questionPlaceholder')}
          aria-label={t('questionAria')}
          onChange={(event) => setQuestion(event.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="flex items-center justify-between gap-2">
          <p className="text-xs text-muted">{t('questionHint')}</p>
          <div className="flex items-center gap-1.5">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              iconOnly
              disabled={busy || !trimmed}
              title={t('understandTopic')}
              aria-label={t('understandTopic')}
              onClick={() => send(onUnderstand)}
            >
              <BulbIcon className="h-3.5 w-3.5" />
            </Button>
            <Button
              type="submit"
              variant="primary"
              size="sm"
              iconOnly
              disabled={busy || !trimmed}
              title={t('sendQuestion')}
              aria-label={t('sendQuestion')}
            >
              <SendIcon className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </form>
    </Section>
  );
}
