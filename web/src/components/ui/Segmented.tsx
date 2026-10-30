/**
 * A segmented control: a few exclusive choices shown side by side (the mode,
 * the theme, the text size). The selected segment is tinted with the accent:
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
    <div role="group" aria-label={label} className="flex shrink-0 items-center gap-0.5 rounded-md bg-raised/60 p-0.5">
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
            className={`flex h-6 items-center gap-1.5 rounded-sm px-2 text-xs transition-colors disabled:opacity-60 ${
              selected ? 'bg-accent-soft text-text ring-1 ring-accent/40' : 'text-muted hover:text-text'
            }`}
          >
            {segment.label}
          </button>
        );
      })}
    </div>
  );
}
