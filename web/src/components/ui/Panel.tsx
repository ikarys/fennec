/**
 * A collapsible section of a drawer: a quiet hairline block with a small icon, title,
 * optional summary, and a rotating chevron. Open state is remembered per section.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { ChevronRightIcon, InfoIcon } from './Icons';
import type { BadgeTone } from './Badge';
import { readStored, writeStored } from '../../lib/storage';

/** The header's block icon carries the block's state; neutral stays quiet. */
const ICON_TONES: Record<BadgeTone, string> = {
  neutral: 'text-muted',
  accent: 'text-accent',
  ok: 'text-ok',
  err: 'text-err',
  attention: 'text-attention',
  info: 'text-info'
};

function storageKey(id: string): string {
  return `fennec.panel.${id}`;
}

function loadOpen(id: string, defaultOpen: boolean): boolean {
  const stored = readStored(storageKey(id));
  return stored === null ? defaultOpen : stored === '1';
}

function saveOpen(id: string, open: boolean): void {
  writeStored(storageKey(id), open ? '1' : '0');
}

export interface PanelProps {
  id: string;
  title: ReactNode;
  summary?: ReactNode;
  defaultOpen?: boolean;
  /** The block's identity icon, shown in place of the leading chevron; it carries `iconTone`. */
  icon?: ReactNode;
  /** The state the block's icon reports; neutral when the block has no state to report. */
  iconTone?: BadgeTone;
  /** What the block is for, behind an info icon next to the title. */
  hint?: string;
  /** Bumping it opens the section and scrolls it into view (a link from elsewhere on the page). */
  revealKey?: number;
  children: ReactNode;
}

export function Panel({ id, title, summary, defaultOpen = true, icon, iconTone = 'neutral', hint, revealKey, children }: PanelProps) {
  const [open, setOpen] = useState(() => loadOpen(id, defaultOpen));
  const detailsRef = useRef<HTMLDetailsElement>(null);

  useEffect(() => {
    if (revealKey === undefined || revealKey === 0) return;
    setOpen(true);
    saveOpen(id, true);
    const frame = requestAnimationFrame(() => detailsRef.current?.scrollIntoView({ block: 'nearest' }));
    return () => cancelAnimationFrame(frame);
  }, [revealKey, id]);

  return (
    <details
      ref={detailsRef}
      open={open}
      onToggle={(event) => {
        const nextOpen = event.currentTarget.open;
        setOpen(nextOpen);
        saveOpen(id, nextOpen);
      }}
      className="group shrink-0 border-b border-border last:border-b-0"
    >
      <summary className="flex cursor-pointer list-none items-center gap-1.5 px-3 pt-3 pb-2 select-none [&::-webkit-details-marker]:hidden hover:text-text">
        <ChevronRightIcon className="h-3 w-3 shrink-0 text-faint transition-transform group-open:rotate-90" />
        {icon !== undefined ? (
          <span aria-hidden className={`shrink-0 ${ICON_TONES[iconTone]}`}>
            {icon}
          </span>
        ) : null}
        <h3 className="m-0 text-[11px] font-semibold uppercase tracking-wide text-muted group-hover:text-text">{title}</h3>
        {hint !== undefined ? (
          <span role="img" title={hint} aria-label={hint} className="cursor-help text-faint hover:text-accent">
            <InfoIcon className="h-3 w-3" />
          </span>
        ) : null}
        {summary !== undefined ? (
          <span className="ml-auto flex items-center gap-1.5 font-mono text-[11px] text-muted tabular-nums">
            {summary}
          </span>
        ) : null}
      </summary>
      <div className="flex flex-col gap-1.5 px-3 pb-3">{children}</div>
    </details>
  );
}
