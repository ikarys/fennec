import type { ModelRef, ProviderListing } from '../../lib/api';
import { Card } from '../ui/Card';
import { FieldRow, Select } from '../ui/Field';

interface ModelSectionProps {
  value: string;
  listings: ProviderListing[];
  stored: ModelRef | null;
  defaultModel: string;
  onChange: (value: string) => void;
}

export function ModelSection({ value, listings, stored, defaultModel, onChange }: ModelSectionProps) {
  const listedValues = new Set<string>();
  for (const listing of listings) {
    for (const model of listing.models) {
      listedValues.add(`${listing.provider}/${model.id}`);
    }
  }

  const unlistedStored =
    stored && !listedValues.has(`${stored.provider}/${stored.model}`) ? stored : null;
  const unlistedProviderName = unlistedStored
    ? listings.find((listing) => listing.provider === unlistedStored.provider)?.name ?? unlistedStored.provider
    : '';

  return (
    <Card className="p-4">
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Modèle</h2>
        <FieldRow label="Modèle">
          <Select value={value} onChange={(event) => onChange(event.target.value)} className="w-full">
            <option value="">Défaut de l'environnement ({defaultModel})</option>
            {listings.map((listing) => (
              <optgroup key={listing.provider} label={listing.name}>
                {listing.models.map((model) => (
                  <option key={`${listing.provider}/${model.id}`} value={`${listing.provider}/${model.id}`}>
                    {model.id}
                  </option>
                ))}
              </optgroup>
            ))}
            {unlistedStored && (
              <option value={`${unlistedStored.provider}/${unlistedStored.model}`}>
                {unlistedStored.model} — {unlistedProviderName} (non listé)
              </option>
            )}
          </Select>
        </FieldRow>
      </div>
    </Card>
  );
}
