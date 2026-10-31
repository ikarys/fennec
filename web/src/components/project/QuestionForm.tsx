import { useState, type FormEvent, type KeyboardEvent } from 'react';
import { FieldRow, TextArea } from '../ui/Field';
import { Button } from '../ui/Button';
import { SendIcon } from '../Icons';

export function QuestionForm({
  busy,
  onAsk,
}: {
  busy: boolean;
  onAsk: (question: string) => void;
}) {
  const [question, setQuestion] = useState('');
  const trimmed = question.trim();

  const submit = () => {
    if (busy || !trimmed) return;
    onAsk(trimmed);
    setQuestion('');
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
    <form onSubmit={onSubmit} className="flex flex-col gap-2">
      <FieldRow label="Poser une question sur le projet" htmlFor="project-question">
        <TextArea
          id="project-question"
          rows={3}
          value={question}
          placeholder="Où est vérifié le Host d'une requête ?"
          onChange={(event) => setQuestion(event.target.value)}
          onKeyDown={onKeyDown}
        />
      </FieldRow>
      <div className="flex justify-end">
        <Button
          type="submit"
          variant="primary"
          size="sm"
          iconOnly
          disabled={busy || !trimmed}
          title="Envoyer la question"
          aria-label="Envoyer la question"
        >
          <SendIcon className="h-3.5 w-3.5" />
        </Button>
      </div>
      <p className="text-xs text-muted">
        Un agent en lecture seule répond, chaque point avec sa source ; il ne modifie rien.
      </p>
    </form>
  );
}
