/** A tab rail: vertical for page sections, horizontal for sub-sections. */
type Tab<T extends string> = { id: T; label: string; count?: number };

export function Tabs<T extends string>({
  tabs,
  active,
  onSelect,
  label,
  orientation = 'vertical',
}: {
  tabs: Tab<T>[];
  active: T;
  onSelect: (id: T) => void;
  label: string;
  orientation?: 'vertical' | 'horizontal';
}) {
  // The count is presentation only; the server owns the number's meaning.
  const renderLabel = (tab: Tab<T>) => (
    <>
      {tab.label}
      {tab.count != null ? (
        <span className="ml-1.5 text-xs text-muted tabular-nums">{tab.count}</span>
      ) : null}
    </>
  );

  if (orientation === 'horizontal') {
    return (
      <div
        role="tablist"
        aria-label={label}
        aria-orientation="horizontal"
        className="flex gap-1 border-b border-border"
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={tab.id === active}
            onClick={() => onSelect(tab.id)}
            className={`-mb-px border-b-2 px-3 py-1.5 text-sm transition-colors ${
              tab.id === active
                ? 'border-accent font-medium text-text'
                : 'border-transparent text-muted hover:text-text'
            }`}
          >
            {renderLabel(tab)}
          </button>
        ))}
      </div>
    );
  }

  return (
    <div
      role="tablist"
      aria-label={label}
      aria-orientation="vertical"
      className="flex w-44 shrink-0 flex-col gap-1"
    >
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={tab.id === active}
          onClick={() => onSelect(tab.id)}
          className={`rounded-md px-3 py-2 text-left text-sm transition-colors ${
            tab.id === active
              ? 'bg-raised font-medium text-text'
              : 'text-muted hover:bg-raised hover:text-text'
          }`}
        >
          {renderLabel(tab)}
        </button>
      ))}
    </div>
  );
}
