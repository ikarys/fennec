import type { ReactNode } from 'react';
import type { SessionUsage } from '../../lib/api';
import { formatSpeed, formatTokens } from '../../lib/format';
import { Panel } from '../ui/Panel';

function Figure({ value, cost }: { value: ReactNode; cost: string | null }) {
  return (
    <>
      {value}
      {cost && <span className="ml-1.5 text-xs text-muted">{cost}</span>}
    </>
  );
}

interface TokensSectionProps {
  /** Computed by the server from the usage ledger; null while the first fetch is in flight. */
  usage: SessionUsage | null;
}

export function TokensSection({ usage }: TokensSectionProps) {
  return (
    <Panel
      id="tokens"
      title="Tokens de la session"
      summary={usage && usage.calls > 0 ? formatTokens(usage.promptTokens + usage.completionTokens) : undefined}
    >
      {(!usage || usage.calls === 0) && <div className="text-xs text-muted">Aucun appel mesuré.</div>}
      {usage && usage.calls > 0 && (
        <dl className="m-0 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-xs tabular-nums">
          <dt className="text-muted">Entrée</dt>
          <dd className="m-0 text-right">
            <Figure value={formatTokens(usage.promptTokens)} cost={usage.inputCost} />
          </dd>
          <dt className="text-muted">dont cache</dt>
          <dd className="m-0 text-right">
            <Figure
              value={`${formatTokens(usage.cachedTokens)} · ${usage.cachePercent} %`}
              cost={usage.cachedCost}
            />
          </dd>
          <dt className="text-muted">Sortie</dt>
          <dd className="m-0 text-right">
            <Figure value={formatTokens(usage.completionTokens)} cost={usage.outputCost} />
          </dd>
          <dt className="text-muted">Coût estimé</dt>
          <dd className="m-0 text-right font-medium" title={usage.costDetail}>
            {usage.cost}
          </dd>
          {usage.lastGenerationSpeed != null && (
            <>
              <dt className="text-muted">Dernier appel</dt>
              <dd className="m-0 text-right">{formatSpeed(usage.lastGenerationSpeed)}</dd>
            </>
          )}
          <dt className="text-muted">Appels</dt>
          <dd className="m-0 text-right">{usage.calls}</dd>
        </dl>
      )}
    </Panel>
  );
}
