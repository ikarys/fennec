/** A vertical rail of tabs: one page split into sections (the project page). */
export function VerticalTabs<T extends string>({
  tabs,
  active,
  onSelect,
  label,
}: {
  tabs: { id: T; label: string }[];
  active: T;
  onSelect: (id: T) => void;
  label: string;
}) {
  return (
    <div role="tablist" aria-label={label} aria-orientation="vertical" className="flex w-44 shrink-0 flex-col gap-1">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={tab.id === active}
          onClick={() => onSelect(tab.id)}
          className={`rounded-md px-3 py-2 text-left text-sm transition-colors ${
            tab.id === active ? 'bg-raised font-medium text-text' : 'text-muted hover:bg-raised hover:text-text'
          }`}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
