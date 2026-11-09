/**
 * The architecture rules the agent proposed, a row of the decision bar: the
 * human appends them to .agents/architecture.
 */
import { useState } from 'react';

import { addArchitectureRules } from '../../lib/api';
import { DecisionRow } from './DecisionBar';
import { LayersIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';

export function RuleProposalDecision({
  proposal,
  sessionId,
  onAdded,
  open,
  onToggle,
}: {
  proposal: { lines: string[]; added: boolean };
  sessionId: string;
  onAdded: () => void;
  open: boolean;
  onToggle: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

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

  const addButton = (
    <Button variant="primary" size="sm" onClick={add} disabled={busy}>
      {t('ruleBridgeAdd')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<LayersIcon className="h-3.5 w-3.5" />}
      title={t('ruleBridgeAddQuestion')}
      meta={`.agents/architecture · ${proposal.lines.length}`}
      primary={addButton}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="flex-1" />
          {addButton}
        </>
      }
    >
      <p className="m-0 mb-2 text-xs text-muted">{t('ruleBridgeAddHint')}</p>
      <pre className="m-0 overflow-x-auto rounded-md bg-code p-2.5 font-mono text-xs text-text whitespace-pre">
        {proposal.lines.join('\n')}
      </pre>
      {error ? <div className="mt-1.5 text-xs text-err">{error}</div> : null}
    </DecisionRow>
  );
}
