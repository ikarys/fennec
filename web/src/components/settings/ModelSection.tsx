import type { SettingsModel } from '../../lib/api';
import { Card } from '../ui/Card';
import { FieldRow, Select } from '../ui/Field';

interface ModelSectionProps {
  value: string;
  models: SettingsModel[];
  storedModel: string | null;
  defaultModel: string;
  onChange: (value: string) => void;
}

export function ModelSection({ value, models, storedModel, defaultModel, onChange }: ModelSectionProps) {
  const listedIds = new Set(models.map((model) => model.id));
  const unlistedStored = storedModel && !listedIds.has(storedModel) ? storedModel : null;

  return (
    <Card className="p-4">
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Modèle</h2>
        <FieldRow label="Modèle">
          <Select value={value} onChange={(event) => onChange(event.target.value)} className="w-full">
            <option value="">Défaut de l'environnement ({defaultModel})</option>
            {models.map((model) => (
              <option key={model.id} value={model.id}>
                {model.id}
              </option>
            ))}
            {unlistedStored && (
              <option value={unlistedStored}>{unlistedStored} (non listé)</option>
            )}
          </Select>
        </FieldRow>
      </div>
    </Card>
  );
}
