/**
 * A collapsible section of a side column: a sticky tinted band with hairlines marks where each
 * block starts. The header stays visible when folded and shows a summary so the state
 * is readable at a glance; open state is remembered per section in localStorage.
 */
import { useState, type ReactNode } from 'react';
import { ChevronRightIcon } from '../Icons';
import { SectionLabel } from './SectionLabel';

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
  children: ReactNode;
}

export function Panel({ id, title, summary, defaultOpen = true, children }: PanelProps) {
  const [open, setOpen] = useState(() => loadOpen(id, defaultOpen));

  return (
    <details
      open={open}
      onToggle={(event) => {
        const nextOpen = event.currentTarget.open;
        setOpen(nextOpen);
        saveOpen(id, nextOpen);
      }}
      className="group shrink-0 -mt-px"
    >
      <summary className="sticky top-0 z-10 flex h-7 cursor-pointer list-none items-center gap-2 border-y border-border bg-raised px-3 select-none hover:bg-border [&::-webkit-details-marker]:hidden">
        <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-open:rotate-90" />
        <SectionLabel className="font-semibold text-text">{title}</SectionLabel>
        <span className="ml-auto flex items-center gap-1.5 font-mono text-xs text-muted tabular-nums">
          {summary}
        </span>
      </summary>
      <div className="flex flex-col gap-1.5 px-3 pt-2 pb-3">{children}</div>
    </details>
  );
}
