import type { ModelRef, ProviderListing } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, Select } from '../ui/Field';

interface ModelSectionProps {
  value: string;
  listings: ProviderListing[];
  stored: ModelRef | null;
  defaultModel: string;
  onChange: (value: string) => void;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
}

export function ModelSection({ value, listings, stored, defaultModel, onChange, save }: ModelSectionProps) {
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
    <Section title="Modèle" save={save}>
      <FieldRow label="Modèle par défaut">
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
      <p className="m-0 text-xs text-muted">
        Les nouvelles sessions l'utilisent ; le composer change celui d'une session. Le modèle par défaut est marqué ★ ; les favoris (♥) sont proposés dans le composer.
      </p>
    </Section>
  );
}
