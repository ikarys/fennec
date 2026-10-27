import { useState } from 'react';

interface PortListEditorProps {
  values: string[];
  onChange: (values: string[]) => void;
  onRename?: (oldValue: string, newValue: string) => void;
  onRemove?: (value: string) => void;
  label?: string;
}

export function PortListEditor({ values, onChange, onRename, onRemove, label }: PortListEditorProps) {
  const [newValue, setNewValue] = useState('');

  const addValue = () => {
    const trimmed = newValue.trim();
    if (trimmed && !values.includes(trimmed)) {
      onChange([...values, trimmed]);
    }
    setNewValue('');
  };

  const removeValue = (value: string) => {
    onChange(values.filter((v) => v !== value));
    onRemove?.(value);
  };

  const renameValue = (index: number, next: string) => {
    const trimmed = next.trim();
    if (!trimmed) return;
    const old = values[index];
    if (trimmed === old) return;
    const nextValues = [...values];
    nextValues[index] = trimmed;
    onChange(nextValues);
    onRename?.(old, trimmed);
  };

  return (
    <div>
      {label && <div className="mb-1 text-[0.8125rem] text-muted">{label}</div>}
      <div className="flex flex-col gap-1">
        {values.map((value, index) => (
          <div key={index} className="flex items-center gap-1">
            <input
              type="text"
              value={value}
              onChange={(event) => renameValue(index, event.target.value)}
              className="grow rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
            />
            <button
              type="button"
              onClick={() => removeValue(value)}
              aria-label="Supprimer"
              className="rounded-md px-1.5 py-1 text-xs text-muted hover:text-err"
            >
              ×
            </button>
          </div>
        ))}
      </div>
      <div className="mt-1 flex items-center gap-1">
        <input
          type="text"
          value={newValue}
          onChange={(event) => setNewValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              addValue();
            }
          }}
          placeholder="Ajouter…"
          className="grow rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
        />
        <button
          type="button"
          onClick={addValue}
          className="rounded-md bg-accent px-2 py-1 text-xs font-medium text-on-accent"
        >
          Ajouter
        </button>
      </div>
    </div>
  );
}
