interface TemplateChipsProps {
  names: string[];
  onInsert: (name: string) => void;
}

export function TemplateChips({ names, onInsert }: TemplateChipsProps) {
  if (names.length === 0) return null;
  return (
    <div className="mt-1 flex flex-wrap gap-1">
      {names.map((name) => (
        <button
          key={name}
          type="button"
          onClick={() => onInsert(name)}
          className="rounded-md border border-border bg-raised px-1.5 py-0.5 text-[11px] text-muted hover:text-text"
        >
          {`{{${name}}}`}
        </button>
      ))}
    </div>
  );
}
