/**
 * The one-click append of the agent's proposed architecture rules to
 * .agents/architecture.
 *
 * Agents cannot write this file: the human confirms and fennec appends the
 * rules in the human's tree, without committing them.
 */
import { useState } from 'react';

import { useT } from '../../i18n';
import { addArchitectureRules } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

export function RuleProposalCard({
  proposal,
  sessionId,
  onAdded,
}: {
  proposal: { lines: string[]; added: boolean };
  sessionId: string | null;
  onAdded: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  if (!sessionId) return null;

  if (proposal.added) {
    return (
      <Card className="p-4">
        <p className="m-0 text-sm text-muted">
          {t('ruleBridgeAdded')}
        </p>
      </Card>
    );
  }

  if (proposal.lines.length === 0) {
    return (
      <Card className="p-4">
        <p className="m-0 text-sm text-muted">
          {t('ruleBridgeNothingNew')}
        </p>
      </Card>
    );
  }

  const add = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await addArchitectureRules(sessionId);
      onAdded();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-1">
        <p className="m-0 text-sm font-medium text-text">{t('ruleBridgeAddQuestion')}</p>
        <p className="m-0 text-xs text-muted">
          {t('ruleBridgeAddHint')}
        </p>
      </div>
      <pre className="m-0 overflow-x-auto rounded-md bg-code p-2.5 font-mono text-xs text-text whitespace-pre">
        {proposal.lines.join('\n')}
      </pre>
      <div className="flex items-center gap-2">
        <Button variant="primary" size="md" onClick={add} disabled={busy}>
          {t('ruleBridgeAdd')}
        </Button>
      </div>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
    </Card>
  );
}
