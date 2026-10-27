/** The small uppercase title of a group (a panel section, a card part). */
import type { ReactNode } from 'react';

export function SectionLabel({ className = '', children }: { className?: string; children: ReactNode }) {
  return (
    <h3 className={`m-0 text-[0.6875rem] font-semibold tracking-[0.06em] text-muted uppercase ${className}`.trim()}>
      {children}
    </h3>
  );
}
