/**
 * The column headings above a list's rows (docs/devs/ui.md, "Never dull"): the tinted
 * row a table-like Card opens with. Callers give the cells, sized like the rows'.
 */
import type { ReactNode } from 'react';

export function TableHeader({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
      {children}
    </div>
  );
}
