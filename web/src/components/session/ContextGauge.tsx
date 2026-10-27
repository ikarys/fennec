import { formatTokens } from '../../lib/format';
import { gaugeTone, type GaugeTone } from '../../lib/contextGauge';
import { Button } from '../ui/Button';

interface ContextGaugeProps {
  currentTokens: number;
  /** The model's context window; 0 when unknown. */
  maxTokens: number;
  contextPercent: number;
  compactAt: number;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
}

const TONE_CLASS: Record<GaugeTone, string> = {
  ok: 'bg-ok',
  warn: 'bg-accent',
  err: 'bg-err'
};

export function ContextGauge({ currentTokens, maxTokens, contextPercent, compactAt, compactDisabled, onCompact, compactError }: ContextGaugeProps) {
  const tone = gaugeTone(contextPercent, compactAt);
  const title =
    maxTokens > 0
      ? `${formatTokens(currentTokens)} pris · ${formatTokens(Math.max(0, maxTokens - currentTokens))} restants · ${formatTokens(maxTokens)} total` +
        (compactAt > 0 ? ` · compaction à ${Math.round(compactAt * 100)} %` : ' · compaction désactivée')
      : `${formatTokens(currentTokens)} tokens`;

  return (
    <div
      role="meter"
      aria-label="Contexte du modèle"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={contextPercent}
      className="flex shrink-0 items-center gap-2"
      title={title}
    >
      <div className="relative h-1.5 w-24 overflow-hidden rounded-full bg-raised">
        <div className={`h-full ${TONE_CLASS[tone]}`} style={{ width: `${contextPercent}%` }} />
        {compactAt > 0 && (
          <span aria-hidden className="absolute inset-y-0 w-px bg-text/60" style={{ left: `${compactAt * 100}%` }} />
        )}
      </div>
      <span className="text-xs text-muted tabular-nums whitespace-nowrap">
        {maxTokens > 0
          ? `${formatTokens(currentTokens)} / ${formatTokens(maxTokens)} · ${contextPercent} %`
          : `${formatTokens(currentTokens)} tokens`}
      </span>
      <Button
        variant="secondary"
        size="sm"
        disabled={compactDisabled}
        onClick={onCompact}
        title="Résumer la conversation pour libérer le contexte"
      >
        Compacter
      </Button>
      {compactError && <span className="text-xs text-err" title={compactError}>échec</span>}
    </div>
  );
}
