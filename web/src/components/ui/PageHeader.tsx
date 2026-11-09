// A page's header: an accent icon tile, the title and its subtitle, the page's actions on the right.
import type { ReactNode } from 'react';
import type { PuzzleIcon } from './Icons';

export function PageHeader({ Icon, title, subtitle, actions }: { Icon: typeof PuzzleIcon; title: ReactNode; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex min-w-0 items-center gap-3">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-accent-soft text-accent">
          <Icon className="h-4 w-4" />
        </span>
        <div className="min-w-0">
          <h1 className="text-2xl font-semibold">{title}</h1>
          {subtitle ? <div className="text-sm text-muted">{subtitle}</div> : null}
        </div>
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}
