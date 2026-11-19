// The default-model picker: a framed section with one model choice row.
import type { ModelRef, ProviderListing } from '../../lib/api';
import { ChoiceMenu, type ChoiceGroup } from '../ui/ChoiceMenu';
import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { StarIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

interface ModelSectionProps {
  value: string;
  listings: ProviderListing[];
  stored: ModelRef | null;
  defaultModel: string;
  onChange: (value: string) => void;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
}

/**
 * The default-model menu's groups, for a `ChoiceMenu`: the environment default
 * first, flush under no band; each provider's models under its name; a stored
 * model the providers no longer list on its own.
 */
export function modelMenuGroups(
  listings: { provider: string; name: string; models: { id: string }[] }[],
  stored: ModelRef | null,
  defaultModel: string,
  translate: (key: MsgKey, vars?: Record<string, string | number>) => string
): ChoiceGroup[] {
  const groups: ChoiceGroup[] = [
    {
      choices: [
        {
          value: '',
          label: defaultModel ? translate('envDefaultOption', { model: defaultModel }) : translate('envDefaultOptionUnnamed')
        }
      ]
    }
  ];
  for (const listing of listings) {
    groups.push({
      label: listing.name,
      choices: listing.models.map((model) => ({ value: `${listing.provider}/${model.id}`, label: model.id }))
    });
  }
  const listed = new Set(groups.flatMap((group) => group.choices.map((choice) => choice.value)));
  if (stored && !listed.has(`${stored.provider}/${stored.model}`)) {
    const providerName = listings.find((listing) => listing.provider === stored.provider)?.name ?? stored.provider;
    groups.push({
      choices: [
        {
          value: `${stored.provider}/${stored.model}`,
          label: translate('unlistedModelOption', { model: stored.model, provider: providerName })
        }
      ]
    });
  }
  return groups;
}

export function ModelSection({ value, listings, stored, defaultModel, onChange, save }: ModelSectionProps) {
  const t = useT();
  const groups = modelMenuGroups(listings, stored, defaultModel, t);
  const label = groups.flatMap((group) => group.choices).find((choice) => choice.value === value)?.label ?? '';

  return (
    <Section Icon={StarIcon} flush title={t('defaultModelLabel')} save={save}>
      <SettingRow label={t('defaultModelLabel')} hint={t('modelSectionHint')}>
        <ChoiceMenu
          label={t('defaultModelLabel')}
          title={label || undefined}
          align="right"
          direction="down"
          width="w-72"
          groups={groups}
          value={value}
          onChange={onChange}
          trigger={<span className="max-w-56 truncate">{label}</span>}
        />
      </SettingRow>
    </Section>
  );
}
