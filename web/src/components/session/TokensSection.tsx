import type { ReactNode } from 'react';
import type { SessionUsage } from '../../lib/api';
import { formatSpeed, formatTokens } from '../../lib/format';
import { CoinIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { Tile } from '../ui/Tile';
import { useLang, useT } from '../../i18n';

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
  const t = useT();
  const lang = useLang();
  const hasUsage = usage !== null && usage.calls > 0;

  return (
    <Panel
      id="tokens"
      title={t('tokensPanelTitle')}
      icon={<CoinIcon className="h-3.5 w-3.5" />}
      summary={hasUsage ? formatTokens(lang, usage.promptTokens + usage.completionTokens) : undefined}
    >
      {(!usage || usage.calls === 0) && <div className="text-xs text-muted">{t('noCallsMeasured')}</div>}
      {hasUsage && (
        <div className="flex flex-col gap-2.5">
          <Tile
            icon={<CoinIcon />}
            figure={usage.cost}
            label={t('estimatedCost')}
            detail={usage.costDetail}
            tone={usage.cost !== '—' ? 'accent' : 'neutral'}
            title={usage.costDetail}
          />
          <div className="flex flex-col gap-1">
            <div className="flex items-baseline justify-between text-xs">
              <span className="text-muted">{t('cacheLabel')}</span>
              <span className="tabular-nums">{usage.cachePercent} %</span>
            </div>
            <div className="h-1 w-full overflow-hidden rounded-full bg-raised">
              <div className="h-full rounded-full bg-ok" style={{ width: `${usage.cachePercent}%` }} />
            </div>
          </div>
          <dl className="m-0 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-xs tabular-nums">
            <dt className="text-muted">{t('inputLabel')}</dt>
            <dd className="m-0 text-right">
              <Figure value={formatTokens(lang, usage.promptTokens)} cost={usage.inputCost} />
            </dd>
            <dt className="text-muted">{t('outputLabel')}</dt>
            <dd className="m-0 text-right">
              <Figure value={formatTokens(lang, usage.completionTokens)} cost={usage.outputCost} />
            </dd>
            {usage.lastGenerationSpeed != null && (
              <>
                <dt className="text-muted">{t('lastCall')}</dt>
                <dd className="m-0 text-right">{formatSpeed(usage.lastGenerationSpeed)}</dd>
              </>
            )}
            <dt className="text-muted">{t('callsLabel')}</dt>
            <dd className="m-0 text-right">{usage.calls}</dd>
          </dl>
        </div>
      )}
    </Panel>
  );
}
