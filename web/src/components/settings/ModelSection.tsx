// The default-model picker: a framed section with one model choice row.
import type { ModelRef, ProviderListing } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { Select } from '../ui/Field';
import { StarIcon } from '../ui/Icons';
import { useT } from '../../i18n';

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
  const t = useT();
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
    <Section Icon={StarIcon} flush title={t('defaultModelLabel')} save={save}>
      <SettingRow label={t('defaultModelLabel')} hint={t('modelSectionHint')}>
        <Select value={value} onChange={(event) => onChange(event.target.value)} className="w-72 max-w-full">
          <option value="">{defaultModel ? t('envDefaultOption', { model: defaultModel }) : t('envDefaultOptionUnnamed')}</option>
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
              {t('unlistedModelOption', { model: unlistedStored.model, provider: unlistedProviderName })}
            </option>
          )}
        </Select>
      </SettingRow>
    </Section>
  );
}
