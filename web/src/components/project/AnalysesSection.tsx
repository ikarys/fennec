import { useState } from 'react';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { PlayIcon, ExternalIcon } from '../Icons';
import { type SessionSummary } from '../../lib/api';
import { QuestionForm } from './QuestionForm';

/** What a project page can ask an agent to analyse: each one opens a session on a read-only agent. */
const ANALYSES = [
  {
    mode: 'kickoff',
    prompt: 'Start this project with me: ask me what it is for, then draft its AGENTS.md and propose its first tasks.',
    title: 'Démarrer le projet',
    description: "Comprend le projet avec toi, rédige un brouillon d'AGENTS.md et propose les premières tâches, dans l'ordre ; aucun code écrit.",
    confirm: "Un agent en lecture seule te pose quelques questions (but, périmètre, contraintes), te fait choisir la stack et l'architecture, puis propose un brouillon d'AGENTS.md et les premières tâches, un squelette qui tourne d'abord. Tu choisis les tâches qui entrent au backlog.",
  },
  {
    mode: 'analyze',
    prompt: 'Analyze this project and draft the philosophy, layers and rules of its AGENTS.md.',
    title: 'Analyser le projet',
    description: "Propose la philosophie, les couches et les règles d'AGENTS.md, chacune sourcée ; tu relis le brouillon.",
    confirm: "Un agent en lecture seule lit le projet et rédige un brouillon d'AGENTS.md (philosophie, couches, règles), chaque point avec sa source. Rien n'est écrit sans toi.",
  },
  {
    mode: 'audit-architecture',
    prompt: 'Audit the software architecture of this project.',
    title: "Auditer l'architecture",
    description: 'Responsabilités, couches, modularité, KISS, SOLID, front ; les constats peuvent devenir des tâches.',
    confirm: "Un agent en lecture seule lit le projet et rapporte ses constats (responsabilités, couches, modularité, KISS, SOLID, front). Sur un gros projet, l'audit peut consommer beaucoup de tokens. Tu choisiras ensuite les constats à transformer en tâches.",
  },
  {
    mode: 'audit-security',
    prompt: 'Audit the security of this project.',
    title: 'Auditer la sécurité',
    description: 'Scanners installés, puis secrets, injections, entrées non bornées, dépendances, CI ; les constats peuvent devenir des tâches.',
    confirm: "Un agent en lecture seule lance les scanners installés (Paramètres, Sécurité) puis lit le code : secrets, injections, entrées non bornées, fuites de ressources, dépendances, CI. Il n'attaque rien et n'installe rien. Sur un gros projet, l'audit peut consommer beaucoup de tokens. Tu choisiras ensuite les constats à transformer en tâches.",
  },
] as const;

type Analysis = (typeof ANALYSES)[number];

export function AnalysesSection({
  busy,
  error,
  sessions,
  onStart,
  onOpenSession,
}: {
  /** A session is being created: every launch waits. */
  busy: boolean;
  error: string | null;
  /** The project's sessions: a running analysis shows as live, not launchable. */
  sessions: SessionSummary[];
  onStart: (prompt: string, mode: string) => void;
  onOpenSession: (sessionId: string) => void;
}) {
  const [pending, setPending] = useState<Analysis | null>(null);

  return (
    <div className="flex flex-col gap-2">
      <QuestionForm busy={busy} onAsk={(question) => onStart(question, 'ask')} />
      <p className="text-xs text-muted">
        Chaque analyse ouvre une session sur un agent en lecture seule : il ne modifie rien, son rapport est une
        proposition.
      </p>
      <ul className="flex flex-col gap-2">
        {ANALYSES.map((analysis) => {
          const running =
            sessions.find((s) => s.mode === analysis.mode && s.state === 'running') ?? null;
          return (
            <li key={analysis.mode} className="flex items-center gap-3 rounded-md border border-border p-3">
              <div className="min-w-0 grow">
                <p className="text-sm font-medium text-text">{analysis.title}</p>
                <p className="text-xs text-muted">{analysis.description}</p>
              </div>
              {running ? (
                <>
                  <Badge tone="accent" dot pulse title="Analyse en cours">
                    En cours
                  </Badge>
                  <Button
                    variant="secondary"
                    size="sm"
                    iconOnly
                    onClick={() => onOpenSession(running.id)}
                    title="Ouvrir la session"
                    aria-label={`Ouvrir ${analysis.title}`}
                  >
                    <ExternalIcon className="h-3.5 w-3.5" />
                  </Button>
                </>
              ) : (
                <Button
                  variant="secondary"
                  size="sm"
                  iconOnly
                  disabled={busy}
                  onClick={() => setPending(analysis)}
                  title={`${analysis.title}…`}
                  aria-label={analysis.title}
                >
                  <PlayIcon className="h-3.5 w-3.5" />
                </Button>
              )}
            </li>
          );
        })}
      </ul>
      {error ? <p className="text-xs text-err">{error}</p> : null}
      <ConfirmDialog
        open={pending !== null}
        title={pending ? `${pending.title} ?` : ''}
        body={pending?.confirm ?? ''}
        confirmLabel="Lancer"
        variant="primary"
        onConfirm={() => {
          if (!pending) return;
          setPending(null);
          onStart(pending.prompt, pending.mode);
        }}
        onCancel={() => setPending(null)}
      />
    </div>
  );
}
