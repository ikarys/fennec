import { useState } from 'react';
import { Button } from '../ui/Button';
import { FieldRow, TextInput } from '../ui/Field';

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

  const content = (
    <>
      <div className="flex flex-col gap-1">
        {values.map((value, index) => (
          <div key={index} className="flex items-center gap-1">
            <TextInput
              type="text"
              value={value}
              onChange={(event) => renameValue(index, event.target.value)}
              className="grow"
            />
            <Button
              type="button"
              variant="ghost"
              size="sm"
              iconOnly
              onClick={() => removeValue(value)}
              aria-label="Supprimer"
            >
              ×
            </Button>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-1">
        <TextInput
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
          className="grow"
        />
        <Button type="button" variant="secondary" size="sm" onClick={addValue}>
          Ajouter
        </Button>
      </div>
    </>
  );

  if (label) {
    return <FieldRow label={label}>{content}</FieldRow>;
  }

  return <div>{content}</div>;
}
