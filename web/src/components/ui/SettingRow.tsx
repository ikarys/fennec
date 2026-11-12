/** A setting of a framed group: what it is on the left, its control on the right; rows sit in a `flush` Section. */
import type { ReactNode } from 'react';

export function SettingRow({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-border px-4 py-2.5 last:border-b-0">
      <div className="min-w-0">
        <div className="text-sm font-medium text-text">{label}</div>
        {hint && <div className="mt-px text-xs text-muted">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
