/**
 * A list on a surface (docs/ui.md, "Never dull"): a Card whose tinted header row
 * names the list (an optional accent icon tile), says what it holds and counts it. `folded` turns the header
 * into the toggle of a list closed by default (content the human rarely opens).
 * Callers give the rows, styled with `listRowClass`.
 */
import type { ReactNode } from 'react';
import { Badge } from './Badge';
import { Card } from './Card';
import { ChevronRightIcon, type PuzzleIcon } from './Icons';

const HEAD = 'flex items-center gap-3 border-border bg-linear-to-r from-accent-soft to-transparent to-75% px-4 py-2.5';
const ROW = 'flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-accent-soft';
const ROW_BAR = 'relative before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:transition-opacity hover:before:opacity-100';

/**
 * A ListCard row's look, for whatever element the row is (button, li, summary, a); callers add layout only.
 * `bar` lights an accent bar on the left on hover; `selected` keeps the row tinted and its bar lit.
 */
export function listRowClass({ bar = false, selected = false }: { bar?: boolean; selected?: boolean } = {}, layout = ''): string {
  const state = selected ? 'bg-accent-soft before:opacity-100' : 'before:opacity-0';
  return `${ROW} ${bar ? `${ROW_BAR} ${state}` : ''} ${layout}`.replace(/\s+/g, ' ').trim();
}

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
