/** The small label of a group (a panel section, a card part). */
import type { ReactNode } from 'react';

export function SectionLabel({
  as = 'h3',
  className = '',
  children,
}: {
  as?: 'h2' | 'h3';
  className?: string;
  children: ReactNode;
}) {
  const Tag = as;
  return (
    <Tag className={`m-0 text-xs font-medium text-muted ${className}`.trim()}>
      {children}
    </Tag>
  );
}
