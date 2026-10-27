/**
 * A small status pill: a run's state, a provenance (verified / indicative),
 * an origin. The tone carries the meaning; the accent is not a status.
 */
import type { ReactNode } from 'react';

export type BadgeTone = 'neutral' | 'accent' | 'ok' | 'err';

const TONES: Record<BadgeTone, string> = {
  neutral: 'bg-raised text-muted',
  accent: 'bg-accent-soft text-accent-strong',
  ok: 'bg-ok-soft text-ok',
  err: 'bg-del-bg text-err'
};

const DOTS: Record<BadgeTone, string> = {
  neutral: 'bg-muted',
  accent: 'bg-accent',
  ok: 'bg-ok',
  err: 'bg-err'
};

export function Badge({
  tone = 'neutral',
  dot = false,
  pulse = false,
  title,
  className = '',
  children
}: {
  tone?: BadgeTone;
  /** A leading dot, for a live state. */
  dot?: boolean;
  /** The dot pulses while something runs. */
  pulse?: boolean;
  title?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <span
      title={title}
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-[0.6875rem] font-medium whitespace-nowrap ${TONES[tone]} ${className}`.trim()}
    >
      {dot && <span className={`h-1.5 w-1.5 rounded-full ${DOTS[tone]} ${pulse ? 'animate-pulse' : ''}`} />}
      {children}
    </span>
  );
}
