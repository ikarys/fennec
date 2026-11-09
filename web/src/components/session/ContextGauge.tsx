import type { ContextView } from '../../lib/api';
import { CompressIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';

interface ContextGaugeProps {
  /** Computed by the server; null until the first fetch. */
  context: ContextView | null;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
  /** Opens the Session drawer on its Context section. */
  onOpen: () => void;
}

const TONE_CLASS: Record<ContextView['tone'], string> = {
  ok: 'bg-ok',
  warn: 'bg-accent',
  err: 'bg-err'
};

export function ContextGauge({ context, compactDisabled, onCompact, compactError, onOpen }: ContextGaugeProps) {
  const t = useT();
  const percent = context?.percent ?? 0;
  return (
    <div
      role="meter"
      aria-label={t('contextOfModel')}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      className="flex shrink-0 items-center gap-2"
    >
      <button
        type="button"
        onClick={onOpen}
        title={context?.title ?? t('contextOfModel')}
        aria-label={t('contextOpenDetail')}
        className="flex items-center gap-2 rounded-md px-1 outline-none hover:bg-raised focus-visible:ring-2 focus-visible:ring-accent-soft"
      >
        <span className="relative block h-1.5 w-24 overflow-hidden rounded-full bg-raised">
          <span className={`block h-full ${TONE_CLASS[context?.tone ?? 'ok']}`} style={{ width: `${percent}%` }} />
          {context?.compactMark != null && (
            <span aria-hidden className="absolute inset-y-0 w-px bg-text/60" style={{ left: `${context.compactMark}%` }} />
          )}
        </span>
        <span className="text-xs text-muted tabular-nums whitespace-nowrap">{context?.line ?? '…'}</span>
      </button>
      <Button
        variant="secondary"
        size="sm"
        iconOnly
        disabled={compactDisabled}
        onClick={onCompact}
        title={t('compactTitle')}
        aria-label={t('compact')}
      >
        <CompressIcon className="h-4 w-4" />
      </Button>
      {compactError && <span className="text-xs text-err" title={compactError}>{t('failure')}</span>}
    </div>
  );
}
