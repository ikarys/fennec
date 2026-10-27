interface TemplateChipsProps {
  names: string[];
  onInsert: (name: string) => void;
}

export function TemplateChips({ names, onInsert }: TemplateChipsProps) {
  if (names.length === 0) return null;
  return (
    // Folded by default: a node late in the graph can offer dozens of names.
    <details className="mt-1">
      <summary className="cursor-pointer text-[0.6875rem] text-muted select-none hover:text-text">
        Variables ({names.length})
      </summary>
      <div className="mt-1 flex max-h-28 flex-wrap gap-1 overflow-y-auto">
        {names.map((name) => (
          <button
            key={name}
            type="button"
            onClick={() => onInsert(name)}
            className="rounded-md border border-border bg-raised px-1.5 py-0.5 text-[0.6875rem] text-muted hover:text-text"
          >
            {`{{${name}}}`}
          </button>
        ))}
      </div>
    </details>
  );
}
