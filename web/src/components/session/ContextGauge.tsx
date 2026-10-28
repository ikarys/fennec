import type { ContextView } from '../../lib/api';
import { Button } from '../ui/Button';

interface ContextGaugeProps {
  /** Computed by the server; null until the first fetch. */
  context: ContextView | null;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
}

const TONE_CLASS: Record<ContextView['tone'], string> = {
  ok: 'bg-ok',
  warn: 'bg-accent',
  err: 'bg-err'
};

export function ContextGauge({ context, compactDisabled, onCompact, compactError }: ContextGaugeProps) {
  const percent = context?.percent ?? 0;
  return (
    <div
      role="meter"
      aria-label="Contexte du modèle"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      className="flex shrink-0 items-center gap-2"
      title={context?.title}
    >
      <div className="relative h-1.5 w-24 overflow-hidden rounded-full bg-raised">
        <div className={`h-full ${TONE_CLASS[context?.tone ?? 'ok']}`} style={{ width: `${percent}%` }} />
        {context?.compactMark != null && (
          <span aria-hidden className="absolute inset-y-0 w-px bg-text/60" style={{ left: `${context.compactMark}%` }} />
        )}
      </div>
      <span className="text-xs text-muted tabular-nums whitespace-nowrap">{context?.line ?? '…'}</span>
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
