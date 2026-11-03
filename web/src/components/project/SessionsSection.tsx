// Full project session list; the home page only shows the last few.
import { useState } from 'react';
import { type SessionSummary } from '../../lib/api';
import { DeleteSessionsDialog } from '../session/DeleteSessionsDialog';
import { EyeIcon, TrashIcon } from '../Icons';
import { SessionStateIcon } from '../session/SessionStateIcon';
import { formatRelative, formatDateTime, stateLabel } from '../../lib/format';
import { Badge } from '../ui/Badge';
import { useLang } from '../../i18n';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section } from '../ui/Section';

export function SessionsSection({
  projectName,
  sessions,
  onOpenSession,
  onSessionsDeleted,
}: {
  projectName: string;
  sessions: SessionSummary[];
  onOpenSession: (id: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
}) {
  const lang = useLang();
  const [pendingDelete, setPendingDelete] = useState<SessionSummary[] | null>(null);
  // Selection mode: a row click toggles it instead of opening the session.
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  // A running session cannot be deleted, so it is never selectable.
  const deletable = sessions.filter((s) => s.state !== 'running');
  const chosen = deletable.filter((s) => selected.has(s.id));
  const allChosen = deletable.length > 0 && chosen.length === deletable.length;

  const toggle = (id: string) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const stopSelecting = () => {
    setSelecting(false);
    setSelected(new Set());
  };

  return (
    <>
      <Section
        title="Sessions"
        description={
          sessions.length === 1
            ? '1 session de ce projet, la dernière créée en haut.'
            : `${sessions.length} sessions de ce projet, la dernière créée en haut.`
        }
        actions={
          deletable.length === 0 ? null : selecting ? (
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted">
                {chosen.length} sélectionnée{chosen.length > 1 ? 's' : ''}
              </span>
              <Button variant="danger" disabled={chosen.length === 0} onClick={() => setPendingDelete(chosen)}>
                <TrashIcon className="h-3.5 w-3.5" />
                Supprimer
              </Button>
              <Button variant="ghost" onClick={stopSelecting}>
                Annuler
              </Button>
            </div>
          ) : (
            <Button variant="secondary" onClick={() => setSelecting(true)}>
              Sélectionner plusieurs
            </Button>
          )
        }
        flush
      >
        {sessions.length === 0 ? (
          <p className="py-3 text-xs text-muted">Aucune session pour l'instant.</p>
        ) : (
          <Card className="overflow-hidden">
            <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
              {selecting ? (
                <input
                  type="checkbox"
                  className="h-3.5 w-3.5 shrink-0 accent-[var(--color-accent)]"
                  aria-label="Tout sélectionner"
                  checked={allChosen}
                  onChange={() => setSelected(allChosen ? new Set() : new Set(deletable.map((s) => s.id)))}
                />
              ) : null}
              <span className="min-w-0 grow">Titre</span>
              <span className="w-28 shrink-0">Agent</span>
              <span className="w-32 shrink-0">Statut</span>
              <span className="w-24 shrink-0 text-right">Modifié</span>
              <span className="w-16 shrink-0" />
            </div>
            <ul className="divide-y divide-border">
              {sessions.map((s) => {
                const running = s.state === 'running';
                const isSelected = selected.has(s.id);
                return (
                  // The whole row opens the session (or toggles it while selecting); the accent edge and tint show which one.
                  <li
                    key={s.id}
                    onClick={() => (selecting ? !running && toggle(s.id) : onOpenSession(s.id))}
                    className={`group relative flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:transition-opacity hover:bg-accent-soft hover:before:opacity-100 ${
                      isSelected ? 'bg-accent-soft before:opacity-100' : 'before:opacity-0'
                    }`}
                  >
                    {selecting ? (
                      <input
                        type="checkbox"
                        className="h-3.5 w-3.5 shrink-0 accent-[var(--color-accent)]"
                        aria-label={`Sélectionner ${s.title}`}
                        checked={isSelected}
                        disabled={running}
                        title={running ? 'Une session en cours ne peut pas être supprimée' : undefined}
                        onClick={(event) => event.stopPropagation()}
                        onChange={() => toggle(s.id)}
                      />
                    ) : null}
                    <span className="min-w-0 grow truncate text-sm text-text group-hover:text-accent-strong" title={s.title}>
                      {s.parentId ? <span className="mr-1 text-xs text-faint">↳</span> : null}
                      {s.title}
                    </span>
                    <span className="w-28 shrink-0">
                      {s.mode ? <Badge tone="accent" pill>{s.mode}</Badge> : null}
                    </span>
                    <span className="flex w-32 shrink-0 flex-col gap-1">
                      <span className="flex items-center gap-1.5 text-xs text-muted">
                        <SessionStateIcon state={s.state} />
                        {stateLabel(lang, s.state)}
                      </span>
                      {s.awaitingIntegration ? <Badge tone="accent" solid>à intégrer</Badge> : null}
                    </span>
                    <span
                      className="w-24 shrink-0 text-right font-mono text-xs text-faint tabular-nums"
                      title={formatDateTime(lang, s.updatedAt)}
                    >
                      {formatRelative(lang, s.updatedAt)}
                    </span>
                    <span className="flex w-16 shrink-0 items-center justify-end gap-1">
                      <Button
                        variant="soft"
                        size="sm"
                        iconOnly
                        title="Ouvrir la session"
                        aria-label="Ouvrir la session"
                        onClick={(event) => {
                          event.stopPropagation();
                          onOpenSession(s.id);
                        }}
                      >
                        <EyeIcon className="h-3.5 w-3.5" />
                      </Button>
                      {running || selecting ? (
                        <span className="w-7 shrink-0" />
                      ) : (
                        <Button
                          variant="danger"
                          size="sm"
                          iconOnly
                          title="Supprimer la session"
                          aria-label="Supprimer la session"
                          onClick={(event) => {
                            event.stopPropagation();
                            setPendingDelete([s]);
                          }}
                        >
                          <TrashIcon className="h-3.5 w-3.5" />
                        </Button>
                      )}
                    </span>
                  </li>
                );
              })}
            </ul>
          </Card>
        )}
      </Section>
      {pendingDelete && (
        <DeleteSessionsDialog
          targets={pendingDelete.map((session) => ({ session, projectName }))}
          onClose={() => setPendingDelete(null)}
          onDeleted={(ids) => {
            // The dialog closes itself, or stays open to list the sessions that failed.
            stopSelecting();
            onSessionsDeleted(ids);
          }}
        />
      )}
    </>
  );
}
