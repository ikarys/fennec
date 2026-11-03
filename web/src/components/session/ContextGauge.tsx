import type { ContextView } from '../../lib/api';
import { CompressIcon } from '../Icons';
import { Button } from '../ui/Button';
import { ContextComposition } from './ContextComposition';

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
    >
      {/* Hover or keyboard focus opens the popover; no state to keep. */}
      <div tabIndex={0} className="group relative flex items-center gap-2 rounded-md outline-none focus-visible:ring-2 focus-visible:ring-accent-soft">
        <div className="relative h-1.5 w-24 overflow-hidden rounded-full bg-raised">
          <div className={`h-full ${TONE_CLASS[context?.tone ?? 'ok']}`} style={{ width: `${percent}%` }} />
          {context?.compactMark != null && (
            <span aria-hidden className="absolute inset-y-0 w-px bg-text/60" style={{ left: `${context.compactMark}%` }} />
          )}
        </div>
        <span className="text-xs text-muted tabular-nums whitespace-nowrap" title={context?.title}>{context?.line ?? '…'}</span>
        {context && (
          <div
            role="tooltip"
            className="invisible absolute top-full left-0 z-30 mt-2 flex w-96 flex-col gap-3 rounded-lg border border-border-strong bg-panel p-4 opacity-0 shadow-float transition-opacity duration-150 group-focus-within:visible group-focus-within:opacity-100 group-hover:visible group-hover:opacity-100"
          >
            <p className="m-0 text-xs text-muted">{context.title}</p>
            {context.composition && <ContextComposition composition={context.composition} />}
          </div>
        )}
      </div>
      <Button
        variant="secondary"
        size="sm"
        iconOnly
        disabled={compactDisabled}
        onClick={onCompact}
        title="Résumer la conversation pour libérer le contexte"
        aria-label="Compacter"
      >
        <CompressIcon className="h-4 w-4" />
      </Button>
      {compactError && <span className="text-xs text-err" title={compactError}>échec</span>}
    </div>
  );
}
