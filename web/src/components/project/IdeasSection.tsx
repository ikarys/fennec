/**
 * The project's ideas backlog: jot an idea, frame it in a plan session, see
 * which are ready. The server folds and sorts the list; this only renders it.
 */
import { useCallback, useEffect, useState, type FormEvent, type KeyboardEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { TextArea } from '../ui/Field';
import {
  addIdea,
  dropIdea,
  frameIdea,
  listIdeas,
  startQueue,
  startIdea,
  stopQueue,
  type Idea,
  type IdeaBoard,
} from '../../lib/ideas';

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
  const [queueRunning, setQueueRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [text, setText] = useState('');
  const [stopping, setStopping] = useState(false);

  const applyBoard = useCallback((board: IdeaBoard) => {
    setIdeas(board.ideas);
    setQueueRunning(board.queueRunning);
  }, []);

  useEffect(() => {
    let cancelled = false;

    setIdeas([]);
    setQueueRunning(false);
    setError(null);

    listIdeas(projectId)
      .then((board) => {
        if (!cancelled) {
          applyBoard(board);
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
  }, [projectId, applyBoard]);

  useEffect(() => {
    if (!queueRunning) {
      setStopping(false);
    }
  }, [queueRunning]);

  const polling =
    queueRunning || ideas.some((idea) => idea.sessionState === 'running');

  useEffect(() => {
    if (!polling) return;

    // The project page has no event stream; the server does no polling, so poll
    // while the queue or a build is running.
    const id = window.setInterval(() => {
      listIdeas(projectId)
        .then(applyBoard)
        .catch(() => {
          // Keep the last known board while a transient poll fails.
        });
    }, 5000);

    return () => window.clearInterval(id);
  }, [polling, projectId, applyBoard]);

  const handleAdd = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const value = text.trim();
    if (!value || busy) return;

    setBusy('add');
    setError(null);

    try {
      const board = await addIdea(projectId, value);
      applyBoard(board);
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
      const board = await frameIdea(projectId, idea.id, sessionId);
      applyBoard(board);
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
      const board = await dropIdea(projectId, idea.id);
      applyBoard(board);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(null);
    }
  };

  const handleStartQueue = async () => {
    if (busy) return;

    setBusy('queue');
    setError(null);

    try {
      const board = await startQueue(projectId);
      applyBoard(board);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(null);
    }
  };

  const handleStartIdea = async (idea: Idea) => {
    setBusy(idea.id);
    setError(null);

    try {
      applyBoard(await startIdea(projectId, idea.id));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(null);
    }
  };

  const handleStopQueue = async () => {
    if (busy) return;

    setBusy('queue');
    setError(null);
    setStopping(true);

    try {
      const board = await stopQueue(projectId);
      applyBoard(board);
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

    if (idea.state === 'ready') {
      return <Badge tone="ok">prête</Badge>;
    }

    switch (idea.sessionState) {
      case 'running':
        return (
          <Badge tone="accent" dot pulse>
            en construction
          </Badge>
        );
      case 'waiting_for_user':
        return <Badge tone="accent">attend ta réponse</Badge>;
      case 'complete':
        return <Badge tone="ok">à relire</Badge>;
      case 'error':
      case 'stopped':
      case 'truncated':
        return <Badge tone="err">à reprendre</Badge>;
      default:
        return <Badge tone="neutral">lancée</Badge>;
    }
  };

  const hasReady = ideas.some((idea) => idea.state === 'ready');

  return (
    <section>
      <div className="flex items-center justify-between gap-3">
        <SectionLabel>Idées ({ideas.length})</SectionLabel>

        {queueRunning ? (
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              disabled={busy !== null}
              onClick={handleStopQueue}
            >
              Arrêter la file
            </Button>

            {stopping ? (
              <span className="text-xs text-muted">
                s'arrête après l'idée en cours
              </span>
            ) : null}
          </div>
        ) : (
          <Button
            variant="secondary"
            size="sm"
            disabled={!hasReady || busy !== null}
            onClick={handleStartQueue}
          >
            Lancer la file
          </Button>
        )}
      </div>

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
                {(idea.state === 'ready' || idea.state === 'started') &&
                idea.plan ? (
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

                {(idea.state === 'framing' ||
                  idea.state === 'ready' ||
                  idea.state === 'started') &&
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

                {idea.state !== 'started' ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled={busy !== null}
                    onClick={() => handleStartIdea(idea)}
                  >
                    Lancer
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
