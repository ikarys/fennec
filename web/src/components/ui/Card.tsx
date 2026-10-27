/**
 * A surface set apart from the page: a dialog body, a plan, a settings group.
 * Only the element that needs to stand out is a card; lists and sections
 * inside one stay flat. Callers add padding and layout.
 */
import type { HTMLAttributes } from 'react';

export function Card({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`rounded-2xl border border-border bg-panel ${className}`.trim()} {...props} />;
}
