/**
 * A collapsible block of a side column. The header stays visible when folded
 * and shows a summary so the state is readable at a glance; open state is
 * remembered per block in localStorage.
 */
import { useState, type ReactNode } from 'react';
import { ChevronRightIcon, PinIcon } from '../Icons';
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
  pinned?: boolean;
  defaultOpen?: boolean;
  children: ReactNode;
}

export function Panel({ id, title, summary, pinned = false, defaultOpen = true, children }: PanelProps) {
  const [open, setOpen] = useState(() => loadOpen(id, defaultOpen));

  return (
    <details
      open={open}
      onToggle={(event) => {
        const nextOpen = event.currentTarget.open;
        setOpen(nextOpen);
        saveOpen(id, nextOpen);
      }}
      className="group shrink-0 overflow-hidden rounded-xl border border-border bg-panel"
    >
      <summary className="flex cursor-pointer list-none items-center gap-2 bg-raised px-3 py-2 select-none [&::-webkit-details-marker]:hidden">
        <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-open:rotate-90" />
        <SectionLabel>{title}</SectionLabel>
        <span className="ml-auto flex items-center gap-1.5 text-xs text-muted tabular-nums">
          {summary}
          {pinned && (
            <span title="Toujours affiché" aria-label="Toujours affiché">
              <PinIcon className="h-3 w-3" />
            </span>
          )}
        </span>
      </summary>
      <div className="flex flex-col gap-1.5 border-t border-border px-3 py-2.5">{children}</div>
    </details>
  );
}
