/**
 * The label of a group, in spaced capitals: an h2 titles a page's group and
 * stands out from its fields; an h3 (a panel section, a card part) stays small and muted.
 */
import type { ReactNode } from 'react';

export function SectionLabel({
  as = 'h3',
  className = '',
  children,
}: {
  as?: 'h2' | 'h3' | 'span';
  className?: string;
  children: ReactNode;
}) {
  const Tag = as;
  const look = as === 'h2' ? 'text-xs font-bold text-text' : 'text-[11px] font-semibold text-muted';
  return (
    <Tag className={`m-0 tracking-wider uppercase ${look} ${className}`.trim()}>
      {children}
    </Tag>
  );
}
