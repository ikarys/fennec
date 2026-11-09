/**
 * The decision bar, docked above the composer: what waits on the human's decision stays one
 * click away while the discussion goes on. Each row shows its primary action folded, and
 * opens in place on its full content and every action; one row open at a time.
 */
import { useEffect, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, type ReactNode } from 'react';
import { ChevronRightIcon } from '../ui/Icons';
import { useT } from '../../i18n';

export function DecisionBar({
  count,
  onEscape,
  children
}: {
  count: number;
  onEscape: () => void;
  children: ReactNode;
}) {
  const t = useT();
  const [unfolded, setUnfolded] = useState(true);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onEscape();
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [onEscape]);

  return (
    // The composer's width and gutter: the bar reads as part of it.
    <div className="shrink-0 px-6 pt-2">
      <div className="mx-auto max-w-page overflow-hidden rounded-lg border border-border border-l-2 border-l-attention bg-panel shadow-float">
        {count > 2 && (
          <button
            type="button"
            onClick={() => setUnfolded((value) => !value)}
            aria-expanded={unfolded}
            className="flex w-full items-center gap-2 bg-attention-soft px-2.5 py-1.5 text-left text-xs hover:bg-attention-soft/80"
          >
            <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-attention" />
            <span className="font-semibold text-attention">{t('decisionsPending', { count })}</span>
            <span className="ml-auto flex items-center">
              <ChevronRightIcon className={`h-3 w-3 text-attention transition-transform ${unfolded ? 'rotate-90' : ''}`} />
            </span>
          </button>
        )}
        <div className={count > 2 && !unfolded ? 'hidden' : undefined}>
          <div className="divide-y divide-border">{children}</div>
        </div>
      </div>
    </div>
  );
}

export function DecisionRow({
  icon,
  title,
  meta,
  primary,
  open,
  onToggle,
  footer,
  danger,
  children
}: {
  icon: ReactNode;
  title: ReactNode;
  meta: ReactNode;
  primary: ReactNode;
  open: boolean;
  onToggle: () => void;
  footer: ReactNode;
  danger?: boolean;
  /** The opened row's content; a row whose title says it all has none. */
  children?: ReactNode;
}) {
  const handleToggle = (event: ReactMouseEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('button')) return;
    onToggle();
  };

  const handleKey = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onToggle();
    }
  };

  // An inset shadow, not a border: the row's content does not move when it turns red.
  return (
    <div className={danger ? 'shadow-[inset_2px_0_0_var(--err)]' : undefined}>
      <div
        role="button"
        tabIndex={0}
        aria-expanded={open}
        onClick={handleToggle}
        onKeyDown={handleKey}
        className={`flex cursor-pointer items-center gap-2.5 px-2.5 py-1.5 hover:bg-accent-soft ${open ? 'bg-accent-soft' : ''}`}
      >
        <ChevronRightIcon
          className={`h-3 w-3 shrink-0 text-faint transition-transform ${open ? 'rotate-90 text-accent' : ''}`}
        />
        <span className="size-6 grid place-items-center rounded-md bg-accent-soft text-accent">{icon}</span>
        <span className="min-w-0 flex-1 flex items-baseline gap-2">
          <span className="truncate font-semibold text-sm text-text">{title}</span>
          <span className="text-xs text-faint whitespace-nowrap">{meta}</span>
        </span>
        {!open && primary}
      </div>
      {open && (
        <div className="border-t border-border">
          {children && <div className="max-h-[40vh] overflow-y-auto py-2.5 pr-3.5 pl-11 text-sm text-prose">{children}</div>}
          <div className={`flex flex-wrap items-center gap-1.5 bg-bg py-1.5 pr-2.5 pl-11 ${children ? 'border-t border-border' : ''}`}>{footer}</div>
        </div>
      )}
    </div>
  );
}
