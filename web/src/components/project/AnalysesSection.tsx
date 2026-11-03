import { useState } from 'react';
import { Badge } from '../ui/Badge';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { Section } from '../ui/Section';
import { AlertIcon, ExternalIcon, LayersIcon, PlayIcon, WandIcon } from '../Icons';
import { type SessionSummary } from '../../lib/api';
import { formatDateTime, formatRelative } from '../../lib/format';
import { QuestionForm } from './QuestionForm';

/** What a project page can ask an agent to analyse: each one opens a session on a read-only agent. */
const ANALYSES = [
  {
    mode: 'analyze',
    prompt: 'Frame this project: read what is there and draft its AGENTS.md. If the workdir is empty, ask me what the project is for and propose its first tasks instead.',
    title: 'Cadrer le projet',
    description:
      "Rédige un brouillon d'AGENTS.md, chaque point sourcé ; sur un dossier vide, te pose les questions et propose les premières tâches.",
    confirm:
      "Un agent en lecture seule lit le projet et rédige un brouillon d'AGENTS.md (philosophie, couches, règles), chaque point avec sa source. Si le dossier est vide, il te pose quelques questions (but, périmètre, contraintes), te fait choisir la stack et l'architecture, puis propose les premières tâches, un squelette qui tourne d'abord. Rien n'est écrit sans toi.",
    icon: WandIcon,
  },
  {
    mode: 'audit-architecture',
    prompt: 'Audit the software architecture of this project.',
    title: "Auditer l'architecture",
    description: 'Responsabilités, couches, modularité, KISS, SOLID, front ; les constats peuvent devenir des tâches.',
    confirm: "Un agent en lecture seule lit le projet et rapporte ses constats (responsabilités, couches, modularité, KISS, SOLID, front). Sur un gros projet, l'audit peut consommer beaucoup de tokens. Tu choisiras ensuite les constats à transformer en tâches.",
    icon: LayersIcon,
  },
  {
    mode: 'audit-security',
    prompt: 'Audit the security of this project.',
    title: 'Auditer la sécurité',
    description: 'Scanners installés, puis secrets, injections, entrées non bornées, dépendances, CI ; les constats peuvent devenir des tâches.',
    confirm: "Un agent en lecture seule lance les scanners installés (Paramètres, Sécurité) puis lit le code : secrets, injections, entrées non bornées, fuites de ressources, dépendances, CI. Il n'attaque rien et n'installe rien. Sur un gros projet, l'audit peut consommer beaucoup de tokens. Tu choisiras ensuite les constats à transformer en tâches.",
    icon: AlertIcon,
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
    <div className="flex flex-col gap-3">
      <QuestionForm busy={busy} onAsk={(question) => onStart(question, 'ask')} />
      <Section
        title="Analyses"
        description="Chaque analyse ouvre une session sur un agent en lecture seule : il ne modifie rien, son rapport est une proposition."
        flush
      >
        <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
          {ANALYSES.map((analysis) => {
            const running =
              sessions.find((session) => session.mode === analysis.mode && session.state === 'running') ?? null;
            const isRunning = running !== null;

            const lastFinished = sessions.reduce<SessionSummary | null>((latest, session) => {
              if (session.mode !== analysis.mode || session.state === 'running') return latest;
              if (!latest) return session;
              return session.updatedAt > latest.updatedAt ? session : latest;
            }, null);

            const Icon = analysis.icon;

            return (
              <button
                key={analysis.mode}
                type="button"
                disabled={busy && !isRunning}
                title={`${analysis.title}…`}
                aria-label={isRunning ? `Ouvrir ${analysis.title}` : analysis.title}
                onClick={() => {
                  if (running) onOpenSession(running.id);
                  else setPending(analysis);
                }}
                className={`group relative flex flex-col gap-3 overflow-hidden rounded-xl border border-border bg-panel p-4 text-left transition-colors hover:border-accent hover:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-60 focus-visible:outline-2 focus-visible:outline-accent ${isRunning ? 'border-accent bg-accent-soft' : ''}`.trim()}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-accent-soft text-accent-strong transition-colors group-hover:ring-1 group-hover:ring-accent">
                    <Icon className="h-4 w-4" />
                  </span>
                  {isRunning ? (
                    <Badge tone="accent" dot pulse title="Analyse en cours">
                      En cours
                    </Badge>
                  ) : null}
                </div>

                <p className="text-sm font-semibold text-text group-hover:text-accent-strong">{analysis.title}</p>
                <p className="text-xs text-muted">{analysis.description}</p>

                <div className="mt-auto flex items-center justify-between gap-2 pt-1">
                  {lastFinished ? (
                    <span
                      role="link"
                      tabIndex={0}
                      title={formatDateTime(lastFinished.updatedAt)}
                      className="text-xs text-faint underline-offset-2 hover:text-accent-strong hover:underline"
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpenSession(lastFinished.id);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.stopPropagation();
                          onOpenSession(lastFinished.id);
                        }
                      }}
                    >
                      Dernière : {formatRelative(lastFinished.updatedAt)}
                    </span>
                  ) : (
                    <span aria-hidden="true" />
                  )}

                  <span className="flex items-center gap-1 text-xs font-medium text-accent-strong">
                    {isRunning ? 'Ouvrir' : 'Lancer'}
                    {isRunning ? <ExternalIcon className="h-3.5 w-3.5" /> : <PlayIcon className="h-3.5 w-3.5" />}
                  </span>
                </div>
              </button>
            );
          })}
        </div>
        {error ? <p className="mt-2 text-xs text-err">{error}</p> : null}
      </Section>
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
