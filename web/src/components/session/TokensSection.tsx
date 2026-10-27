import type { ThreadView } from '../../lib/fold';
import { cachePercent, formatSpeed, formatTokens } from '../../lib/format';
import { Panel } from '../ui/Panel';

interface TokensSectionProps {
  usage: ThreadView['usage'];
  partial: boolean;
}

export function TokensSection({ usage, partial }: TokensSectionProps) {
  return (
    <Panel
      id="tokens"
      title={partial ? 'Tokens des messages chargés' : 'Tokens de la session'}
      summary={usage.calls > 0 ? formatTokens(usage.prefillTokens + usage.generationTokens) : undefined}
    >
      {usage.calls === 0 && <div className="text-xs text-muted">Aucun appel mesuré.</div>}
      {usage.calls > 0 && (
        <dl className="m-0 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-xs tabular-nums">
          <dt className="text-muted">Entrée</dt>
          <dd className="m-0 text-right">{formatTokens(usage.prefillTokens)}</dd>
          <dt className="text-muted">dont cache</dt>
          <dd className="m-0 text-right">
            {formatTokens(usage.cachedPromptTokens)} · {cachePercent(usage.cachedPromptTokens, usage.prefillTokens)} %
          </dd>
          <dt className="text-muted">Sortie</dt>
          <dd className="m-0 text-right">{formatTokens(usage.generationTokens)}</dd>
          {usage.last && (
            <>
              <dt className="text-muted">Dernier appel</dt>
              <dd className="m-0 text-right">{formatSpeed(usage.last.generationSpeed)}</dd>
            </>
          )}
          <dt className="text-muted">Appels</dt>
          <dd className="m-0 text-right">{usage.calls}</dd>
        </dl>
      )}
    </Panel>
  );
}
