// A settings-page note: an info icon and one line of explanatory text.
import type { ReactNode } from 'react';
import { InfoIcon } from './Icons';

export function InfoBar({ children }: { children: ReactNode }) {
  return (
    <p className="m-0 flex items-start gap-2 rounded-lg border border-border bg-panel px-3 py-2 text-xs text-muted">
      <InfoIcon className="mt-px h-3.5 w-3.5 shrink-0 text-accent" />
      <span>{children}</span>
    </p>
  );
}
