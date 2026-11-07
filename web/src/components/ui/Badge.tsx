/**
 * A small status badge: a run's state, a provenance (verified / indicative),
 * an origin. The tone carries the meaning; the accent is not a status.
 */
import type { ReactNode } from 'react';

export type BadgeTone = 'neutral' | 'accent' | 'ok' | 'err' | 'attention' | 'info';

/**
 * How the badge reads: `text` coloured text; `pill` outlined and rounded, a tag rather
 * than text; `solid` an accent fill, for what the eye must catch in a long list (a live
 * count, a branch to integrate).
 */
export type BadgeAppearance = 'text' | 'pill' | 'solid';

/** Colour says the state; only what needs the human (accent) gets a background. */
const TONES: Record<BadgeTone, string> = {
  neutral: 'text-muted',
  accent: 'bg-accent-soft text-accent-strong',
  ok: 'text-ok',
  err: 'text-err',
  attention: 'text-attention',
  info: 'text-info'
};

/** A pill always carries its tone: a tinted border and background. */
const PILLS: Record<BadgeTone, string> = {
  neutral: 'border-border bg-raised text-muted',
  accent: 'border-accent/40 bg-accent-soft text-accent-strong',
  ok: 'border-ok/40 bg-ok-soft text-ok',
  err: 'border-err/40 bg-err-soft text-err',
  attention: 'border-attention/40 bg-attention-soft text-attention',
  info: 'border-info/40 bg-info-soft text-info'
};

const SHAPES: Record<BadgeAppearance, (tone: BadgeTone) => string> = {
  text: (tone) => `rounded-sm px-1.5 ${TONES[tone]}`,
  pill: (tone) => `rounded-full border px-2 ${PILLS[tone]}`,
  solid: () => 'rounded-sm bg-accent px-1.5 text-on-accent'
};

const DOTS: Record<BadgeTone, string> = {
  neutral: 'bg-muted',
  accent: 'bg-accent',
  ok: 'bg-ok',
  err: 'bg-err',
  attention: 'bg-attention',
  info: 'bg-info'
};

export function Badge({
  tone = 'neutral',
  dot = false,
  pulse = false,
  appearance = 'text',
  title,
  className = '',
  children
}: {
  tone?: BadgeTone;
  /** A leading dot, for a live state. */
  dot?: boolean;
  /** The dot pulses while something runs. */
  pulse?: boolean;
  appearance?: BadgeAppearance;
  title?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <span
      title={title}
      className={`inline-flex shrink-0 items-center gap-1.5 py-0.5 text-xs font-medium whitespace-nowrap ${SHAPES[appearance](tone)} ${className}`.trim()}
    >
      {dot && <span className={`h-1.5 w-1.5 rounded-full ${appearance === 'solid' ? 'bg-on-accent' : DOTS[tone]} ${pulse ? 'animate-pulse' : ''}`} />}
      {children}
    </span>
  );
}
