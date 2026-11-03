/**
 * A collapsible section of a side column: a sticky title on a raised background, a
 * hairline between blocks marks where each starts. The header stays visible when folded and shows a summary so the state
 * is readable at a glance; open state is remembered per section in localStorage.
 */
import { useState, type ReactNode } from 'react';
import { ChevronRightIcon } from '../Icons';
import type { BadgeTone } from './Badge';
import { SectionLabel } from './SectionLabel';

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
  try {
    const stored = localStorage.getItem(storageKey(id));
    return stored === null ? defaultOpen : stored === '1';
  } catch {
    return defaultOpen;
  }
}

function saveOpen(id: string, open: boolean): void {
  try {
    localStorage.setItem(storageKey(id), open ? '1' : '0');
  } catch {
    // A blocked storage only loses the state on reload.
  }
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
  children: ReactNode;
}

export function Panel({ id, title, summary, defaultOpen = true, icon, iconTone = 'neutral', children }: PanelProps) {
  const [open, setOpen] = useState(() => loadOpen(id, defaultOpen));

  return (
    <details
      open={open}
      onToggle={(event) => {
        const nextOpen = event.currentTarget.open;
        setOpen(nextOpen);
        saveOpen(id, nextOpen);
      }}
      className="group shrink-0 border-t border-border first:border-t-0"
    >
      <summary className="sticky top-0 z-10 flex h-8 cursor-pointer list-none items-center gap-2 bg-raised px-3 select-none hover:bg-accent-soft [&::-webkit-details-marker]:hidden">
        {icon !== undefined ? (
          <span aria-hidden className={`shrink-0 ${ICON_TONES[iconTone]}`}>
            {icon}
          </span>
        ) : (
          <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-hover:text-text group-open:rotate-90" />
        )}
        <SectionLabel>{title}</SectionLabel>
        <span className="ml-auto flex items-center gap-1.5 font-mono text-xs text-muted tabular-nums">
          {summary}
        </span>
        <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-open:rotate-90" />
      </summary>
      <div className="flex flex-col gap-1.5 px-3 pt-2 pb-3">{children}</div>
    </details>
  );
}
