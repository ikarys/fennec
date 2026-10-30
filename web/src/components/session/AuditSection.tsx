import { useState } from 'react';

import { createAuditTasks, type Audit } from '../../lib/api';
import { panelBlock } from '../../lib/panelBlocks';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Panel } from '../ui/Panel';

const SEVERITY = {
  high: { label: 'haute', tone: 'err' },
  medium: { label: 'moyenne', tone: 'neutral' },
  low: { label: 'basse', tone: 'neutral' },
} as const;

export function AuditSection({
  audit,
  sessionId,
  projectId,
}: {
  audit: Audit | null;
  sessionId: string | null;
  projectId: string | null;
}) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [created, setCreated] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!audit || audit.findings.length === 0 || !sessionId) return null;

  const allChecked = audit.findings.every((finding) => checked.has(finding.number));

  const toggle = (number: number) => {
    setChecked((previous) => {
      const next = new Set(previous);
      if (next.has(number)) next.delete(number);
      else next.add(number);
      return next;
    });
  };

  const toggleAll = () => {
    setChecked(allChecked ? new Set() : new Set(audit.findings.map((finding) => finding.number)));
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
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel id="audit" title={panelBlock('audit').title} summary={audit.findings.length}>
      <p className="m-0 text-[0.6875rem] text-muted">
        Constats de l'agent (indicatif). Coche ceux à garder : chacun devient une tâche du projet.
      </p>
      <ul className="m-0 flex list-none flex-col gap-2 p-0">
        {audit.findings.map((finding) => (
          <li key={finding.number}>
            <label className="flex cursor-pointer items-start gap-2">
              <input
                type="checkbox"
                className="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-accent)]"
                checked={checked.has(finding.number)}
                disabled={busy}
                onChange={() => toggle(finding.number)}
              />
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="flex items-center gap-1.5">
                  <Badge tone={SEVERITY[finding.severity].tone} dot>
                    {SEVERITY[finding.severity].label}
                  </Badge>
                  <span className="text-xs font-medium text-text">{finding.title}</span>
                </span>
                <span className="text-[0.6875rem] text-muted line-clamp-2" title={finding.problem}>
                  {finding.problem}
                </span>
                {finding.locations.length > 0 ? (
                  <span
                    className="truncate font-mono text-[0.6875rem] text-faint"
                    title={finding.locations.join('\n')}
                  >
                    {finding.locations.join(', ')}
                  </span>
                ) : null}
              </div>
            </label>
          </li>
        ))}
      </ul>
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={toggleAll} disabled={busy}>
          {allChecked ? 'Tout décocher' : 'Tout cocher'}
        </Button>
        <Button
          variant="primary"
          size="sm"
          onClick={handleCreate}
          disabled={checked.size === 0 || busy}
          title="Ajoute les constats cochés au backlog du projet"
        >
          Créer {checked.size} tâche{checked.size > 1 ? 's' : ''}
        </Button>
      </div>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      {created !== null ? (
        <div className="flex items-center gap-2">
          <p className="m-0 text-xs text-ok">{created} tâche(s) ajoutée(s) au projet.</p>
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
    </Panel>
  );
}
