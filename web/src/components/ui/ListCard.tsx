/**
 * A list on a surface (docs/ui.md, "Never dull"): a Card whose tinted header row
 * names the list (an optional accent icon tile), says what it holds and counts it. `folded` turns the header
 * into the toggle of a list closed by default (content the human rarely opens).
 * Callers give the rows, each with `px-4`.
 */
import type { ReactNode } from 'react';
import { Badge } from './Badge';
import { Card } from './Card';
import { ChevronRightIcon, type PuzzleIcon } from './Icons';

const HEAD = 'flex items-center gap-3 border-border bg-linear-to-r from-accent-soft to-transparent to-75% px-4 py-2.5';

export function ListCard({
  Icon,
  title,
  description,
  count,
  folded = false,
  children,
}: {
  Icon?: typeof PuzzleIcon;
  title: ReactNode;
  description?: ReactNode;
  /** What the badge shows: a number, or a ratio like `3 / 8`. */
  count?: ReactNode;
  folded?: boolean;
  children: ReactNode;
}) {
  const head = (
    <>
      {Icon ? (
        <span className="grid size-6 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
          <Icon className="h-3.5 w-3.5" />
        </span>
      ) : null}
      <div className="min-w-0 grow">
        <div className="text-sm font-semibold text-text">{title}</div>
        {description ? <div className="text-xs text-muted">{description}</div> : null}
      </div>
      {count !== undefined ? (
        <Badge tone="neutral" appearance="pill">
          {count}
        </Badge>
      ) : null}
    </>
  );
  return (
    <Card className="overflow-hidden">
      {folded ? (
        <details className="group">
          <summary
            className={`${HEAD} cursor-pointer list-none select-none hover:bg-accent-soft group-open:border-b [&::-webkit-details-marker]:hidden`}
          >
            <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-open:rotate-90" />
            {head}
          </summary>
          {children}
        </details>
      ) : (
        <>
          <div className={`${HEAD} border-b`}>{head}</div>
          {children}
        </>
      )}
    </Card>
  );
}
