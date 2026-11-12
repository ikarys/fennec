import type { ContextView } from '../../lib/api';
import { CompressIcon } from '../ui/Icons';
import { SectionLabel } from '../ui/SectionLabel';
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
  const compactTone =
    context?.tone === 'warn' || context?.tone === 'err'
      ? 'text-accent bg-accent-soft hover:bg-accent/25'
      : 'text-muted hover:bg-raised hover:text-text';
  return (
    <div className="flex shrink-0 items-center gap-2">
      <div
        role="meter"
        aria-label={t('contextOfModel')}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        className="flex shrink-0 items-stretch overflow-hidden rounded-lg border border-border bg-raised/40"
      >
        <button
          type="button"
          onClick={onOpen}
          title={context?.title ?? t('contextOfModel')}
          aria-label={t('contextOpenDetail')}
          className="flex items-center gap-2 px-2.5 py-1 outline-none transition-colors hover:bg-raised focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-soft"
        >
          <SectionLabel as="span">{t('contextShort')}</SectionLabel>
          <span className="relative block h-1.5 w-24 overflow-hidden rounded-full bg-raised">
            <span className={`block h-full ${TONE_CLASS[context?.tone ?? 'ok']}`} style={{ width: `${percent}%` }} />
            {context?.compactMark != null && (
              <span aria-hidden className="absolute inset-y-0 w-px bg-text/60" style={{ left: `${context.compactMark}%` }} />
            )}
          </span>
          <span className="text-xs text-muted tabular-nums whitespace-nowrap">{context?.line ?? '…'}</span>
        </button>
        <span aria-hidden className="w-px self-stretch bg-border" />
        <button
          type="button"
          disabled={compactDisabled}
          onClick={onCompact}
          title={t('compactTitle')}
          aria-label={t('compact')}
          className={`flex items-center px-2 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-soft disabled:pointer-events-none disabled:text-faint ${compactTone}`}
        >
          <CompressIcon className="h-3.5 w-3.5" />
        </button>
      </div>
      {compactError && <span className="text-xs text-err" title={compactError}>{t('failure')}</span>}
    </div>
  );
}
