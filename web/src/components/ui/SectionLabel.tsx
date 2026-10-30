/** The small label of a group (a panel section, a card part). */
import type { ReactNode } from 'react';

export function SectionLabel({ className = '', children }: { className?: string; children: ReactNode }) {
  return (
    <h3 className={`m-0 text-xs font-medium text-muted ${className}`.trim()}>
      {children}
    </h3>
  );
}
