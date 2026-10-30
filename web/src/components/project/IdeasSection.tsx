/**
 * The project's ideas backlog: jot an idea, frame it in a plan session, see
 * which are ready. The server folds and sorts the list; this only renders it.
 */
import { useEffect, useState, type FormEvent, type KeyboardEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { TextArea } from '../ui/Field';
import { addIdea, dropIdea, frameIdea, listIdeas, type Idea } from '../../lib/ideas';

export function IdeasSection({
  projectId,
  onFrame,
  onOpenSession,
}: {
  projectId: string;
  onFrame: (idea: Idea) => Promise<string>;
  onOpenSession: (sessionId: string) => void;
}) {
  const [ideas, setIdeas] = useState<Idea[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [text, setText] = useState('');

  useEffect(() => {
    let cancelled = false;

    setIdeas([]);
    setError(null);

    listIdeas(projectId)
      .then((list) => {
        if (!cancelled) {
          setIdeas(list);
          setError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Erreur inconnue');
        }
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const handleAdd = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const value = text.trim();
    if (!value || busy) return;

    setBusy('add');
    setError(null);

    try {
      const list = await addIdea(projectId, value);
      setIdeas(list);
      setText('');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(null);
    }
  };

  const handleFrame = async (idea: Idea) => {
    setBusy(idea.id);
    setError(null);

    try {
      const sessionId = await onFrame(idea);
      const list = await frameIdea(projectId, idea.id, sessionId);
      setIdeas(list);
      onOpenSession(sessionId);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(null);
    }
  };

  const handleDrop = async (idea: Idea) => {
    if (!window.confirm('Abandonner cette idée ?')) return;

    setBusy(idea.id);
    setError(null);

    try {
      const list = await dropIdea(projectId, idea.id);
      setIdeas(list);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(null);
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      if (!text.trim() || busy) return;
      event.currentTarget.form?.requestSubmit();
    }
  };

  const badgeFor = (idea: Idea) => {
    if (idea.state === 'inbox') {
      return <Badge tone="neutral">à cadrer</Badge>;
    }

    if (idea.state === 'framing') {
      return <Badge tone="accent">en cadrage</Badge>;
    }

    return <Badge tone="ok">prête</Badge>;
  };

  return (
    <section>
      <SectionLabel>Idées ({ideas.length})</SectionLabel>

      <form className="mt-3" onSubmit={handleAdd}>
        <TextArea
          rows={2}
          className="w-full"
          placeholder="Une idée à garder pour plus tard…"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={handleKeyDown}
        />
        <div className="mt-2 flex justify-end">
          <Button
            type="submit"
            variant="secondary"
            size="sm"
            disabled={!text.trim() || busy !== null}
          >
            Ajouter
          </Button>
        </div>
      </form>

      {error ? <p className="mt-1 text-xs text-err">{error}</p> : null}

      {ideas.length === 0 ? (
        <p className="mt-3 text-xs text-muted">Aucune idée pour l'instant.</p>
      ) : (
        <Card className="mt-3">
          {ideas.map((idea, index) => (
            <div
              key={idea.id}
              className={`flex items-start gap-3 px-4 py-3 text-sm ${
                index > 0 ? 'border-t border-border' : ''
              }`}
            >
              <div className="grow whitespace-pre-wrap">
                {idea.text}
                {idea.state === 'ready' && idea.plan ? (
                  <details className="mt-2">
                    <summary className="cursor-pointer text-xs text-muted">
                      Plan approuvé
                    </summary>
                    <pre className="mt-1 whitespace-pre-wrap text-xs text-text">
                      {idea.plan}
                    </pre>
                  </details>
                ) : null}
              </div>

              <div className="shrink-0">{badgeFor(idea)}</div>

              <div className="flex shrink-0 items-center gap-2">
                {idea.state === 'inbox' ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled={busy !== null}
                    onClick={() => handleFrame(idea)}
                  >
                    Cadrer
                  </Button>
                ) : null}

                {(idea.state === 'framing' || idea.state === 'ready') &&
                idea.sessionId ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={busy !== null}
                    onClick={() => idea.sessionId && onOpenSession(idea.sessionId)}
                  >
                    Ouvrir la session
                  </Button>
                ) : null}

                {idea.state === 'framing' ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled={busy !== null}
                    onClick={() => handleFrame(idea)}
                  >
                    Recadrer
                  </Button>
                ) : null}

                <Button
                  variant="ghost"
                  size="sm"
                  disabled={busy !== null}
                  onClick={() => handleDrop(idea)}
                >
                  Abandonner
                </Button>
              </div>
            </div>
          ))}
        </Card>
      )}
    </section>
  );
}
