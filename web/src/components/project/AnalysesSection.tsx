// Project page analyses tab: a question form and launchable project analyses.
import { useState } from 'react';
import { Badge } from '../ui/Badge';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { Section } from '../ui/Section';
import { AlertIcon, ExternalIcon, LayersIcon, PlayIcon, SearchIcon, WandIcon } from '../ui/Icons';
import { type SessionSummary } from '../../lib/api';
import { formatDateTime, formatRelative } from '../../lib/format';
import { QuestionForm } from './QuestionForm';
import { useLang, useT } from '../../i18n';

/** What a project page can ask an agent to analyse: each one opens a session on a read-only agent. */
const ANALYSES = [
  {
    mode: 'analyze',
    prompt: 'Frame this project: read what is there and draft its AGENTS.md. If the workdir is empty, ask me what the project is for and propose its first tasks instead.',
    title: 'analyzeTitle',
    description: 'analyzeDescription',
    confirm: 'analyzeConfirm',
    icon: WandIcon,
  },
  {
    mode: 'audit-architecture',
    prompt: 'Audit the software architecture of this project.',
    title: 'auditArchTitle',
    description: 'auditArchDescription',
    confirm: 'auditArchConfirm',
    icon: LayersIcon,
  },
  {
    mode: 'audit-security',
    prompt: 'Audit the security of this project.',
    title: 'auditSecurityTitle',
    description: 'auditSecurityDescription',
    confirm: 'auditSecurityConfirm',
    icon: AlertIcon,
  },
] as const;

type Analysis = (typeof ANALYSES)[number];

/** The understand agent's brief on a part of the project (not a branch: the current code). */
function understandTopicPrompt(topic: string): string {
  return `Hold a comprehension dialogue with me on this part of the project: ${topic}\nRead the current code (no branch to diff), then ask me your questions.`;
}

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
  const lang = useLang();
  const t = useT();
  const [pending, setPending] = useState<Analysis | null>(null);

  return (
    <div className="flex flex-col gap-3">
      <QuestionForm
        busy={busy}
        onAsk={(question) => onStart(question, 'ask')}
        onUnderstand={(topic) => onStart(understandTopicPrompt(topic), 'understand')}
      />
      <Section
        Icon={SearchIcon}
        title={t('analysesTitle')}
        description={t('analysesDescription')}
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
                title={`${t(analysis.title)}…`}
                aria-label={isRunning ? t('openAnalysis', { title: t(analysis.title) }) : t(analysis.title)}
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
                    <Badge tone="accent" dot pulse title={t('analysisRunning')}>
                      {t('running')}
                    </Badge>
                  ) : null}
                </div>

                <p className="text-sm font-semibold text-text group-hover:text-accent-strong">{t(analysis.title)}</p>
                <p className="text-xs text-muted">{t(analysis.description)}</p>

                <div className="mt-auto flex items-center justify-between gap-2 pt-1">
                  {lastFinished ? (
                    <span
                      role="link"
                      tabIndex={0}
                      title={formatDateTime(lang, lastFinished.updatedAt)}
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
                      {t('lastLabel', { when: formatRelative(lang, lastFinished.updatedAt) })}
                    </span>
                  ) : (
                    <span aria-hidden="true" />
                  )}

                  <span className="flex items-center gap-1 text-xs font-medium text-accent-strong">
                    {isRunning ? t('openAction') : t('launchAction')}
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
        title={pending ? `${t(pending.title)} ?` : ''}
        body={pending ? t(pending.confirm) : ''}
        confirmLabel={t('launchAction')}
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
