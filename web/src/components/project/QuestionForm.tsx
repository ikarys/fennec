import { useState, type FormEvent, type KeyboardEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { TextArea } from '../ui/Field';
import { ReplyIcon, SendIcon } from '../Icons';

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
    <Card className="overflow-hidden">
      <div className="flex items-center gap-2 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
        <ReplyIcon className="h-3.5 w-3.5 text-accent" />
        <span>Poser une question</span>
        <span className="ml-auto">
          <Badge tone="info" pill title="Réponse produite par un modèle, chaque point sourcé">
            indicatif
          </Badge>
        </span>
      </div>

      <form onSubmit={onSubmit} className="flex flex-col gap-3 p-4">
        <TextArea
          id="project-question"
          rows={3}
          value={question}
          placeholder="Où est vérifié le Host d'une requête ?"
          aria-label="Poser une question sur le projet"
          onChange={(event) => setQuestion(event.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="flex items-center justify-between gap-2">
          <p className="text-xs text-muted">
            Un agent en lecture seule répond, chaque point avec sa source ; il ne modifie rien.
          </p>
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
      </form>
    </Card>
  );
}
