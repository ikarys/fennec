import { Button } from '../ui/Button';
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
          <Button key={name} variant="ghost" className="font-mono" onClick={() => onInsert(name)}>
            {`{{${name}}}`}
          </Button>
        ))}
      </div>
    </details>
  );
}
