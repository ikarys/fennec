/**
 * A segmented control: a few exclusive choices shown side by side (the mode,
 * the theme, the text size), framed and split by hairlines. The selected segment is filled with the accent:
 * a lighter background alone vanishes in the dark themes, where --panel is
 * darker than --raised.
 */
import type { ReactNode } from 'react';

export interface Segment<T> {
  value: T;
  label: ReactNode;
  title?: string;
  ariaLabel?: string;
}

export function Segmented<T extends string | number>({
  label,
  segments,
  value,
  onChange,
  disabled
}: {
  /** The group's accessible name. */
  label: string;
  segments: Segment<T>[];
  value: T | null;
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div role="group" aria-label={label} className="flex shrink-0 divide-x divide-border overflow-hidden rounded-2xl border border-border">
      {segments.map((segment) => {
        const selected = segment.value === value;
        return (
          <button
            key={segment.value}
            type="button"
            title={segment.title}
            aria-label={segment.ariaLabel}
            aria-pressed={selected}
            disabled={disabled}
            onClick={() => onChange(segment.value)}
            className={`flex h-7 min-w-0 items-center gap-1.5 px-3 text-xs transition-colors disabled:opacity-60 ${
              selected ? 'bg-accent text-on-accent' : 'text-muted hover:bg-raised hover:text-text'
            }`}
          >
            {/* Long labels (user-set panel page names) truncate instead of clipping the segment.
                A hidden medium-weight copy holds the width: selecting a segment does not move its neighbours. */}
            <span className="grid min-w-0">
              <span className={`col-start-1 row-start-1 truncate ${selected ? 'font-medium' : ''}`}>{segment.label}</span>
              <span aria-hidden="true" className="invisible col-start-1 row-start-1 truncate font-medium">{segment.label}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}
