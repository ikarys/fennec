/**
 * A group of a settings page: a bordered block whose header is a tinted band (title, an
 * optional one-line description, actions on the right), the same band as a side column's
 * `Panel`, so where a group starts is seen at a glance; the body is optional.
 */
import type { ReactNode } from 'react';

export interface SectionProps {
  title: ReactNode;
  description?: ReactNode;
  /** Header buttons (`Button size="sm"`, often `iconOnly`): what the group lets the human add. */
  actions?: ReactNode;
  className?: string;
  /** The body has no padding: rows (a list) run edge to edge. */
  flush?: boolean;
  children?: ReactNode;
}

export function Section({ title, description, actions, className = '', flush = false, children }: SectionProps) {
  return (
    <section className={`overflow-hidden rounded-md border border-border bg-panel ${className}`.trim()}>
      <header className={`flex min-h-10 items-center gap-3 ${children ? 'border-b border-border ' : ''}bg-raised px-4 py-1.5`}>
        <div className="flex min-w-0 flex-col">
          <h2 className="m-0 text-sm font-semibold">{title}</h2>
          {description && <p className="m-0 text-xs text-muted">{description}</p>}
        </div>
        {actions && <div className="ml-auto flex shrink-0 items-center gap-1">{actions}</div>}
      </header>
      {children && <div className={flush ? 'flex flex-col' : 'flex flex-col gap-4 p-4'}>{children}</div>}
    </section>
  );
}
