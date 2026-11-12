// The providers settings list: an optional add-provider draft and one card per provider.
import { useState } from 'react';
import type { ModelRef, ProviderListing, SettingsDefaults } from '../../lib/api';
import { Button } from '../ui/Button';
import { Section, type SectionSave } from '../ui/Section';
import { PlusIcon } from '../ui/Icons';
import { FieldRow, TextInput } from '../ui/Field';
import { type ModelForm } from '../../lib/settingsForm';
import { ProviderCard } from './ProviderCard';
import { useT } from '../../i18n';

export interface ProviderForm {
  id: string;
  name: string;
  baseUrl: string;
  keyKind: 'none' | 'env' | 'stored';
  envVar: string;
  newKey: string;
  sealed: string | null;
}

interface ProviderDraft {
  id: string;
  name: string;
  baseUrl: string;
}

const emptyDraft: ProviderDraft = { id: '', name: '', baseUrl: '' };

interface ProvidersSectionProps {
  providers: ProviderForm[];
  listings: ProviderListing[];
  /** The form's per-model entries: those absent from a listing were added by hand. */
  values: Record<string, ModelForm>;
  billing: Record<string, boolean>;
  defaults: SettingsDefaults;
  favorites: ModelRef[];
  /** `provider/model` of the default model, '' when none is chosen. */
  defaultModel: string;
  adding: boolean;
  onAddingChange: (adding: boolean) => void;
  onChange: (index: number, patch: Partial<ProviderForm>) => void;
  onAdd: (provider: ProviderForm) => void;
  onRemove: (index: number) => void;
  onFetch: (provider: ProviderForm) => Promise<void>;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
  onAddModel: (providerId: string, modelId: string) => void;
  onToggleFavorite: (ref: ModelRef) => void;
  onToggleBilled: (key: string, billed: boolean) => void;
  onSetDefault: (key: string) => void;
  onChangeModel: (key: string, field: keyof ModelForm, value: string) => void;
  onRemoveModel: (providerId: string, modelId: string) => void;
}

export function ProvidersSection({
  providers,
  listings,
  values,
  billing,
  defaults,
  favorites,
  defaultModel,
  adding,
  onAddingChange,
  onChange,
  onAdd,
  onRemove,
  onFetch,
  save,
  onAddModel,
  onToggleFavorite,
  onToggleBilled,
  onSetDefault,
  onChangeModel,
  onRemoveModel
}: ProvidersSectionProps) {
  const t = useT();
  const [draft, setDraft] = useState<ProviderDraft>(emptyDraft);
  const draftId = draft.id.trim();
  const draftDisabled =
    draftId === '' || draftId.includes('/') || providers.some((provider) => provider.id === draftId);

  function addProvider() {
    if (draftDisabled) return;
    onAdd({
      id: draftId,
      name: draft.name.trim(),
      baseUrl: draft.baseUrl.trim(),
      keyKind: 'none',
      envVar: '',
      newKey: '',
      sealed: null
    });
    setDraft(emptyDraft);
    onAddingChange(false);
  }

  return (
    <div className="flex flex-col gap-6">
      {adding && (
        <Section Icon={PlusIcon} title={t('addProvider')}>
          <div className="grid gap-2 md:grid-cols-3">
            <FieldRow label={t('idLabel')} hint={t('idHint')}>
              <TextInput
                value={draft.id}
                onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('nameLabel')}>
              <TextInput
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('baseUrlLabel')}>
              <TextInput
                value={draft.baseUrl}
                onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
                className="w-full"
              />
            </FieldRow>
          </div>
          <div className="flex items-center gap-2">
            <Button type="button" variant="secondary" size="sm" disabled={draftDisabled} onClick={addProvider}>
              {t('add')}
            </Button>
            <Button type="button" variant="ghost" size="sm" onClick={() => onAddingChange(false)}>
              {t('cancel')}
            </Button>
            <p className="m-0 text-xs text-faint">{t('providerAddHint')}</p>
          </div>
        </Section>
      )}

      {providers.map((provider, index) => (
        <ProviderCard
          key={provider.id}
          provider={provider}
          index={index}
          listings={listings}
          values={values}
          billing={billing}
          defaults={defaults}
          favorites={favorites}
          defaultModel={defaultModel}
          save={save}
          onChange={onChange}
          onRemove={onRemove}
          onFetch={onFetch}
          onAddModel={onAddModel}
          onToggleFavorite={onToggleFavorite}
          onToggleBilled={onToggleBilled}
          onSetDefault={onSetDefault}
          onChangeModel={onChangeModel}
          onRemoveModel={onRemoveModel}
        />
      ))}
    </div>
  );
}
