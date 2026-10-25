import type { AssistantItem } from '../../lib/fold';
import { cachePercent, formatDuration, formatSpeed, formatTokens } from '../../lib/format';

/** Under each answer: the live speed while it streams, then what the server measured. */
export function MessageStatsLine({ item, streaming }: { item: AssistantItem; streaming: boolean }) {
  if (streaming) {
    if (item.liveSpeed == null) return null;
    return (
      <div className="text-[11px] text-muted tabular-nums" title="Estimé d'après le texte reçu, mesuré à la fin du message">
        ≈ {formatSpeed(item.liveSpeed)}
      </div>
    );
  }
  const stats = item.stats;
  if (!stats) return null;
  const fresh = stats.prefillTokens - stats.cachedPromptTokens;
  return (
    <div
      className="flex min-w-0 gap-x-3 overflow-hidden text-[11px] whitespace-nowrap text-muted tabular-nums"
      title={`Entrée ${stats.prefillTokens} tokens dont ${stats.cachedPromptTokens} en cache, ${fresh} nouveaux à ${formatSpeed(stats.prefillSpeed)} · sortie ${stats.generationTokens} tokens`}
    >
      <span>↑ {formatTokens(stats.prefillTokens)}</span>
      <span>cache {cachePercent(stats.cachedPromptTokens, stats.prefillTokens)} %</span>
      <span>↓ {formatTokens(stats.generationTokens)}</span>
      <span>{formatSpeed(stats.generationSpeed)}</span>
      <span>{formatDuration(Math.round(stats.totalTime * 1000))}</span>
    </div>
  );
}
