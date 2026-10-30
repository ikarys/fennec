/**
 * A group of a page (a settings group) set apart by a hairline above it,
 * not a box; the first one of its parent has none.
 */
import type { HTMLAttributes } from 'react';

export function Section({ className = '', ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={`flex flex-col gap-4 border-t border-border pt-6 first:border-t-0 first:pt-0 ${className}`.trim()} {...props} />;
}
