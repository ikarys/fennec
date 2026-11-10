/** A tab rail: vertical for page sections, horizontal for sub-sections. */
import type { ReactNode } from 'react';
import type { PuzzleIcon } from './Icons';

type Tab<T extends string> = { id: T; label: string; count?: number; Icon?: typeof PuzzleIcon };

export function Tabs<T extends string>({
  tabs,
  active,
  onSelect,
  label,
  trailing,
  orientation = 'vertical',
}: {
  tabs: Tab<T>[];
  active: T;
  onSelect: (id: T) => void;
  label: string;
  orientation?: 'vertical' | 'horizontal';
  /** Shown at the right end of a horizontal rail: the section's own action. */
  trailing?: ReactNode;
}) {
  // The count is presentation only; the server owns the number's meaning.
  const renderLabel = (tab: Tab<T>) => (
    <>
      {tab.label}
      {tab.count != null ? (
        <span
          className={`rounded-full px-1.5 py-0.5 text-xs tabular-nums ${
            tab.id === active ? 'bg-accent-soft text-accent-strong' : 'bg-raised text-muted'
          }`}
        >
          {tab.count}
        </span>
      ) : null}
    </>
  );

  if (orientation === 'horizontal') {
    return (
      <div
        role="tablist"
        aria-label={label}
        aria-orientation="horizontal"
        className="flex items-end gap-0.5 border-b border-border"
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={tab.id === active}
            onClick={() => onSelect(tab.id)}
            className={`relative -mb-px flex h-9 items-center gap-2 whitespace-nowrap rounded-t-lg border border-b-0 px-3 text-sm transition-colors ${
              tab.id === active
                ? 'border-border bg-bg font-medium text-text'
                : 'border-transparent text-muted hover:bg-raised/50 hover:text-text'
            }`}
          >
            {tab.id === active ? <span aria-hidden className="absolute inset-x-2 top-0 h-0.5 rounded-b bg-accent" /> : null}
            {tab.Icon ? <tab.Icon className={`h-3.5 w-3.5 shrink-0 ${tab.id === active ? 'text-accent' : ''}`} /> : null}
            {renderLabel(tab)}
          </button>
        ))}
        {trailing ? <div className="ml-auto self-center">{trailing}</div> : null}
      </div>
    );
  }

  return (
    <div
      role="tablist"
      aria-label={label}
      aria-orientation="vertical"
      className="flex shrink-0 gap-px overflow-x-auto md:sticky md:top-6 md:w-56 md:flex-col md:overflow-visible"
    >
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={tab.id === active}
          onClick={() => onSelect(tab.id)}
          className={`relative flex h-[34px] items-center gap-2.5 whitespace-nowrap rounded-md pl-2 pr-2.5 text-left text-sm transition-colors ${
            tab.id === active
              ? 'bg-accent-soft font-semibold text-accent before:absolute before:-left-0.5 before:top-[7px] before:bottom-[7px] before:w-[2.5px] before:rounded-sm before:bg-accent'
              : 'text-muted hover:bg-raised hover:text-text'
          }`}
        >
          {tab.Icon ? <tab.Icon className="h-4 w-4 shrink-0" /> : null}
          {renderLabel(tab)}
        </button>
      ))}
    </div>
  );
}
