import type { ReactNode } from 'react';
import type { SessionUsage } from '../../lib/api';
import { formatSpeed, formatTokens } from '../../lib/format';
import { CoinIcon } from '../Icons';
import { Panel } from '../ui/Panel';
import { Tile } from '../ui/Tile';

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
  const hasUsage = usage !== null && usage.calls > 0;

  return (
    <Panel
      id="tokens"
      title="Tokens de la session"
      summary={hasUsage ? formatTokens(usage.promptTokens + usage.completionTokens) : undefined}
    >
      {(!usage || usage.calls === 0) && <div className="text-xs text-muted">Aucun appel mesuré.</div>}
      {hasUsage && (
        <div className="flex flex-col gap-2.5">
          <Tile
            icon={<CoinIcon />}
            figure={usage.cost}
            label="Coût estimé"
            detail={usage.costDetail}
            tone={usage.cost !== '—' ? 'accent' : 'neutral'}
            title={usage.costDetail}
          />
          <div className="flex flex-col gap-1">
            <div className="flex items-baseline justify-between text-xs">
              <span className="text-muted">Cache</span>
              <span className="tabular-nums">{usage.cachePercent} %</span>
            </div>
            <div className="h-1 w-full overflow-hidden rounded-full bg-raised">
              <div className="h-full rounded-full bg-ok" style={{ width: `${usage.cachePercent}%` }} />
            </div>
          </div>
          <dl className="m-0 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-xs tabular-nums">
            <dt className="text-muted">Entrée</dt>
            <dd className="m-0 text-right">
              <Figure value={formatTokens(usage.promptTokens)} cost={usage.inputCost} />
            </dd>
            <dt className="text-muted">Sortie</dt>
            <dd className="m-0 text-right">
              <Figure value={formatTokens(usage.completionTokens)} cost={usage.outputCost} />
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
        </div>
      )}
    </Panel>
  );
}
