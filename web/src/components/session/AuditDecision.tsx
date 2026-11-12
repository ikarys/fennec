/**
 * The audit findings not taken yet, a row of the decision bar: the human
 * picks which ones become backlog tasks.
 */
import { useState } from 'react';

import { createAuditTasks, type Audit } from '../../lib/api';
import { DecisionRow } from './DecisionBar';
import { ReviewIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

const SEVERITY: Record<'high' | 'medium' | 'low', { label: MsgKey; tone: 'err' | 'neutral' }> = {
  high: { label: 'severityHigh', tone: 'err' },
  medium: { label: 'severityMedium', tone: 'neutral' },
  low: { label: 'severityLow', tone: 'neutral' },
};

export function AuditDecision({
  audit,
  sessionId,
  projectId,
  onCreated,
  open,
  onToggle,
}: {
  audit: Audit;
  sessionId: string;
  projectId: string | null;
  onCreated: () => void;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [created, setCreated] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const taken = new Set(audit.taken);
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
      setError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setBusy(false);
    }
  };

  const reviewButton = (
    <Button variant="secondary" size="sm" onClick={onToggle}>
      {t('decisionReview')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<ReviewIcon className="h-3.5 w-3.5" />}
      title={t('whichFindingsQuestion')}
      meta={`${t('auditBadge')} · ${untaken.length}`}
      primary={reviewButton}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={toggleAll} disabled={busy}>
            {allChecked ? t('uncheckAll') : t('checkAll')}
          </Button>
          <span className="flex-1" />
          <Button
            variant="primary"
            size="sm"
            onClick={handleCreate}
            disabled={checked.size === 0 || busy}
            title={t('createFindingsTitle')}
          >
            {checked.size === 0 ? t('createTasks') : t('createTasksCount', { count: checked.size })}
          </Button>
        </>
      }
    >
      <p className="m-0 mb-2 text-xs text-muted">{t('findingsHint')}</p>
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
                  className="h-4 w-4 shrink-0"
                  checked={isTaken || checked.has(finding.number)}
                  disabled={busy || isTaken}
                  onChange={() => toggle(finding.number)}
                />
                <span className="tabular-nums text-xs text-muted">{finding.number}.</span>
                <Badge tone={SEVERITY[finding.severity].tone} dot>
                  {t(SEVERITY[finding.severity].label)}
                </Badge>
                <span className="min-w-0 text-text">{finding.title}</span>
                {isTaken ? <Badge tone="ok">{t('taskCreatedBadge')}</Badge> : null}
              </label>
            </li>
          );
        })}
      </ul>
      {created !== null ? (
        <div className="mt-1.5 flex items-center gap-2">
          <p className="m-0 text-xs text-ok">{t('tasksAddedNote', { count: created })}</p>
          {projectId ? (
            <a
              href={'#/project/' + encodeURIComponent(projectId) + '/tasks'}
              className="text-xs text-accent-strong underline"
            >
              {t('viewTasks')}
            </a>
          ) : null}
        </div>
      ) : null}
      {error ? <div className="mt-1.5 text-xs text-err">{error}</div> : null}
    </DecisionRow>
  );
}
