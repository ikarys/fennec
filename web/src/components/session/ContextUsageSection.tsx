import { formatTokens } from '../../lib/format';
import { panelBlock } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { Panel } from '../ui/Panel';

interface ContextUsageSectionProps {
  currentTokens: number;
  /** The model's context window; 0 when unknown. */
  maxTokens: number;
  contextPercent: number;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
}

export function ContextUsageSection({ currentTokens, maxTokens, contextPercent, compactDisabled, onCompact, compactError }: ContextUsageSectionProps) {
  // The gauge turns warm as compaction nears (it triggers at 85 % by default).
  const gaugeColor = contextPercent < 70 ? 'bg-ok' : contextPercent < 85 ? 'bg-accent' : 'bg-err';
  return (
    <Panel id="context" title={panelBlock('context').title} summary={`${contextPercent} %`}>
      <div
        className="h-1.5 overflow-hidden rounded-full bg-raised"
        title={
          maxTokens > 0
            ? `${formatTokens(currentTokens)} pris · ${formatTokens(Math.max(0, maxTokens - currentTokens))} restants · ${formatTokens(maxTokens)} total`
            : `${formatTokens(currentTokens)} tokens`
        }
      >
        <div className={`h-full ${gaugeColor}`} style={{ width: `${contextPercent}%` }} />
      </div>
      <div className="flex justify-between text-xs text-muted tabular-nums">
        <span>{maxTokens > 0 ? `${formatTokens(currentTokens)} / ${formatTokens(maxTokens)}` : formatTokens(currentTokens)} tokens</span>
        <span>{contextPercent} %</span>
      </div>
      <Button
        variant="secondary"
        size="sm"
        onClick={onCompact}
        disabled={compactDisabled}
        className="self-start"
      >
        Compacter
      </Button>
      {compactError && <div className="text-xs text-err">{compactError}</div>}
    </Panel>
  );
}
