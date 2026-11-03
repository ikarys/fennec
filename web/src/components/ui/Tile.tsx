/**
 * A dashboard tile: an icon, a label, a figure, and an optional detail.
 * It renders a div, or a button when an onClick handler is provided.
 */
import type { ReactNode } from 'react';
import type { BadgeTone } from './Badge';

const SHELL =
  'group relative flex flex-col gap-3 overflow-hidden rounded-md border px-4 py-3.5 text-left transition-colors';

const INTERACTIVE =
  'cursor-pointer hover:border-border-strong focus-visible:outline-2 focus-visible:outline-accent';

const BARS: Record<BadgeTone, string> = {
  accent: 'bg-accent',
  ok: 'bg-ok',
  err: 'bg-err',
  attention: 'bg-attention',
  neutral: 'bg-border-strong',
  info: 'bg-info'
};

const ICON_TONES: Record<BadgeTone, string> = {
  accent: 'bg-accent-soft text-accent-strong',
  ok: 'bg-ok-soft text-ok',
  err: 'bg-raised text-err',
  attention: 'bg-raised text-attention',
  neutral: 'bg-raised text-muted',
  info: 'bg-raised text-info'
};

const ACTIVE_BORDERS: Record<BadgeTone, string> = {
  accent: 'border-accent-soft',
  ok: 'border-border',
  err: 'border-border',
  attention: 'border-border',
  neutral: 'border-border',
  info: 'border-border'
};

const ACTIVE_WASHES: Record<BadgeTone, string> = {
  accent: 'bg-linear-to-br from-accent-soft/60 to-panel',
  ok: 'bg-linear-to-br from-ok-soft/50 to-panel',
  err: 'bg-linear-to-br from-raised to-panel',
  attention: 'bg-linear-to-br from-raised to-panel',
  neutral: 'bg-linear-to-br from-raised to-panel',
  info: 'bg-linear-to-br from-raised to-panel'
};

const ACTIVE_FIGURES: Record<BadgeTone, string> = {
  accent: 'text-accent-strong',
  ok: 'text-ok',
  err: 'text-err',
  attention: 'text-attention',
  neutral: 'text-text',
  info: 'text-info'
};

export function Tile({
  icon,
  figure,
  label,
  detail,
  tone = 'neutral',
  active = false,
  pulse = false,
  title,
  onClick
}: {
  icon: ReactNode;
  figure: number | string;
  label: string;
  detail?: string;
  tone?: BadgeTone;
  active?: boolean;
  pulse?: boolean;
  title?: string;
  onClick?: () => void;
}) {
  const stateClass = active
    ? `${ACTIVE_BORDERS[tone]} ${ACTIVE_WASHES[tone]}`
    : 'border-border bg-panel';

  const className = `${SHELL} ${stateClass}${onClick ? ` ${INTERACTIVE}` : ''}`;

  const content = (
    <>
      {active ? (
        <span aria-hidden="true" className={`absolute inset-y-0 left-0 w-[3px] ${BARS[tone]}`} />
      ) : null}

      <div className="flex items-center gap-2.5">
        <span
          className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-md [&>svg]:h-4 [&>svg]:w-4 ${ICON_TONES[tone]}`}
        >
          {icon}
        </span>
        <span className="text-sm font-medium text-text">{label}</span>
        {pulse ? <span className="ml-auto h-2 w-2 rounded-full animate-pulse bg-accent" /> : null}
      </div>

      <div className="flex flex-col gap-1.5">
        <span
          className={`text-3xl font-semibold leading-none tabular-nums ${
            active ? ACTIVE_FIGURES[tone] : 'text-text'
          }`}
        >
          {figure}
        </span>
        {detail ? <span className="max-w-full truncate text-xs text-muted">{detail}</span> : null}
      </div>
    </>
  );

  if (onClick) {
    return (
      <button type="button" title={title} onClick={onClick} className={className}>
        {content}
      </button>
    );
  }

  return (
    <div title={title} className={className}>
      {content}
    </div>
  );
}
