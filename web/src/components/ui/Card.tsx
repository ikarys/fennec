/**
 * A surface set apart from the page: a dialog body, a plan.
 * Only the element that needs to stand out is a card; lists and sections
 * inside one stay flat. Callers add padding and layout.
 * `inset`: one entry of a group's repeated items (a panel page, a provider), a tinted block
 * with its own padding, no border.
 */
import type { HTMLAttributes } from 'react';

const VARIANTS = {
  surface: 'rounded-xl border border-border bg-panel',
  inset: 'flex flex-col gap-2 rounded-md bg-raised p-3',
} as const;

export function Card({
  variant = 'surface',
  className = '',
  ...props
}: HTMLAttributes<HTMLDivElement> & { variant?: keyof typeof VARIANTS }) {
  return <div className={`${VARIANTS[variant]} ${className}`.trim()} {...props} />;
}
