/**
 * The audit findings, offered as tasks inside the thread.
 *
 * It sits right after the report so the human sees the proposed tasks where
 * they read the audit, not hidden in the right-hand context panel.
 */
import { useState } from 'react';

import { createAuditTasks, type Audit } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

const SEVERITY = {
  high: { label: 'haute', tone: 'err' },
  medium: { label: 'moyenne', tone: 'neutral' },
  low: { label: 'basse', tone: 'neutral' },
} as const;

export function AuditCard({
  audit,
  sessionId,
  projectId,
  onCreated,
}: {
  audit: Audit | null;
  sessionId: string | null;
  projectId: string | null;
  onCreated: () => void;
}) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [created, setCreated] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!audit || audit.findings.length === 0 || !sessionId) return null;

  const taken = new Set(audit.taken);
  const allTaken = audit.findings.every((finding) => taken.has(finding.number));

  if (allTaken) {
    const count = audit.findings.length;
    return (
      <Card className="p-4">
        <p className="m-0 flex items-center gap-2 text-sm text-muted">
          {count === 1
            ? 'Le constat de cet audit est dans le backlog.'
            : `Les ${count} constats de cet audit sont dans le backlog.`}
          {projectId ? (
            <a
              href={'#/project/' + encodeURIComponent(projectId) + '/tasks'}
              className="text-xs text-accent-strong underline"
            >
              Voir les tâches
            </a>
          ) : null}
        </p>
      </Card>
    );
  }

  const untaken = audit.findings
    .filter((finding) => !taken.has(finding.number))
    .map((finding) => finding.number);
  const allChecked = untaken.length > 0 && untaken.every((number) => checked.has(number));

  const toggle = (number: number) => {
    setChecked((previous) => {
      const next = new Set(previous);
      if (next.has(number)) next.delete(number);
      else next.add(number);
      return next;
    });
  };

  const toggleAll = () => {
    setChecked(allChecked ? new Set() : new Set(untaken));
  };

  const handleCreate = async () => {
    if (checked.size === 0 || busy) return;
    setBusy(true);
    setError(null);
    setCreated(null);
    try {
      const result = await createAuditTasks(sessionId, [...checked].sort((a, b) => a - b));
      setCreated(result.created);
      setChecked(new Set());
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-1">
        <p className="m-0 text-sm font-medium text-text">Garder quels constats ?</p>
        <p className="m-0 text-xs text-muted">
          Proposés par l'agent (indicatif) : chaque constat coché devient une tâche du projet, que tu cadreras ensuite.
        </p>
      </div>
      <ul className="m-0 flex list-none flex-col gap-2 p-0">
        {audit.findings.map((finding) => {
          const isTaken = taken.has(finding.number);
          return (
            <li key={finding.number}>
              <label
                className={'flex items-center gap-2 text-sm' + (isTaken ? '' : ' cursor-pointer')}
                title={finding.problem}
              >
                <input
                  type="checkbox"
                  className="h-4 w-4 shrink-0 accent-[var(--color-accent)]"
                  checked={isTaken || checked.has(finding.number)}
                  disabled={busy || isTaken}
                  onChange={() => toggle(finding.number)}
                />
                <span className="tabular-nums text-xs text-muted">{finding.number}.</span>
                <Badge tone={SEVERITY[finding.severity].tone} dot>
                  {SEVERITY[finding.severity].label}
                </Badge>
                <span className="min-w-0 text-text">{finding.title}</span>
                {isTaken ? <Badge tone="ok">tâche créée</Badge> : null}
              </label>
            </li>
          );
        })}
      </ul>
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={toggleAll} disabled={busy}>
          {allChecked ? 'Tout décocher' : 'Tout cocher'}
        </Button>
        <Button
          variant="primary"
          size="md"
          onClick={handleCreate}
          disabled={checked.size === 0 || busy}
          title="Ajoute les constats cochés au backlog du projet"
        >
          {checked.size === 0
            ? 'Créer les tâches'
            : `Créer ${checked.size} tâche${checked.size > 1 ? 's' : ''}`}
        </Button>
      </div>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      {created !== null ? (
        <div className="flex items-center gap-2">
          <p className="m-0 text-xs text-ok">{created} tâche(s) ajoutée(s) au backlog.</p>
          {projectId ? (
            <a
              href={'#/project/' + encodeURIComponent(projectId) + '/tasks'}
              className="text-xs text-accent-strong underline"
            >
              Voir les tâches
            </a>
          ) : null}
        </div>
      ) : null}
    </Card>
  );
}
