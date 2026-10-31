/**
 * A run metric (duration, tokens, counts) as a compact chip;
 * the full figure goes in the title.
 */
import type { ReactNode } from 'react';

const toneClasses = {
  neutral: 'text-muted',
  ok: 'text-ok',
  err: 'text-err',
} as const;

export function Stat({
  icon,
  title,
  tone = 'neutral',
  children,
}: {
  icon: ReactNode;
  title?: string;
  tone?: 'neutral' | 'ok' | 'err';
  children: ReactNode;
}) {
  return (
    <span
      title={title}
      className={`inline-flex shrink-0 items-center gap-1 rounded-md border border-border bg-raised px-1.5 py-0.5 text-xs leading-4 tabular-nums whitespace-nowrap ${toneClasses[tone]}`}
    >
      <span className="opacity-80 [&>svg]:h-3 [&>svg]:w-3" aria-hidden="true">
        {icon}
      </span>
      {children}
    </span>
  );
}
