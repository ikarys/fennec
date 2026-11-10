// A framed card for one provider: its connection form, model list, and row actions.
import { useState } from 'react';
import type { ModelRef, ProviderListing, SettingsDefaults } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Section, type SectionSave } from '../ui/Section';
import { CloseIcon, EditIcon, PlusIcon, RefreshIcon, ServerIcon, SpinnerIcon, TrashIcon } from '../ui/Icons';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { formOnlyModels, type ModelForm } from '../../lib/settingsForm';
import { ModelRow } from './ModelRow';
import type { ProviderForm } from './ProvidersSection';
import { useT } from '../../i18n';

interface ProviderCardProps {
  provider: ProviderForm;
  index: number;
  listings: ProviderListing[];
  values: Record<string, ModelForm>;
  billing: Record<string, boolean>;
  defaults: SettingsDefaults;
  favorites: ModelRef[];
  defaultModel: string;
  save?: SectionSave;
  onChange: (index: number, patch: Partial<ProviderForm>) => void;
  onRemove: (index: number) => void;
  onFetch: (provider: ProviderForm) => Promise<void>;
  onAddModel: (providerId: string, modelId: string) => void;
  onToggleFavorite: (ref: ModelRef) => void;
  onToggleBilled: (key: string, billed: boolean) => void;
  onSetDefault: (key: string) => void;
  onChangeModel: (key: string, field: keyof ModelForm, value: string) => void;
  onRemoveModel: (providerId: string, modelId: string) => void;
}

export function ProviderCard({
  provider,
  index,
  listings,
  values,
  billing,
  defaults,
  favorites,
  defaultModel,
  save,
  onChange,
  onRemove,
  onFetch,
  onAddModel,
  onToggleFavorite,
  onToggleBilled,
  onSetDefault,
  onChangeModel,
  onRemoveModel
}: ProviderCardProps) {
  const t = useT();
  const [editing, setEditing] = useState(false);
  const [addingModel, setAddingModel] = useState(false);
  const [modelDraft, setModelDraft] = useState('');
  const [fetching, setFetching] = useState(false);

  const listing = listings.find((item) => item.provider === provider.id);
  const isLocal = provider.id === 'local';
  const error = listing?.error ?? null;
  const added = formOnlyModels(values, provider.id, listing);
  const count = (listing?.models.length ?? 0) + added.length;
  const showConnection = editing || provider.baseUrl === '';

  function addModel() {
    const modelId = modelDraft.trim();
    if (modelId === '') return;
    onAddModel(provider.id, modelId);
    setModelDraft('');
  }

  return (
    <Section
      Icon={ServerIcon}
      flush
      title={<span className="font-mono">{provider.name || provider.id}</span>}
      description={<span className="font-mono">{provider.baseUrl}</span>}
      save={save}
      actions={
        <>
          {error ? (
            <Badge tone="err" title={error}>
              {t('providerError')}
            </Badge>
          ) : (
            <Badge tone={count === 0 ? 'neutral' : 'info'} appearance="pill">
              {count === 0 ? t('noModels') : t('modelCount', { count })}
            </Badge>
          )}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-label={t('addModelTitle')}
            title={t('addModelHint')}
            onClick={() => setAddingModel((current) => !current)}
          >
            <PlusIcon className="h-3.5 w-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            disabled={fetching}
            aria-label={t('fetchModels')}
            title={fetching ? t('fetchingModels') : t('fetchModelsTitle')}
            onClick={() => {
              void (async () => {
                setFetching(true);
                try {
                  await onFetch(provider);
                } catch {
                  // A failed fetch leaves the listing as it was.
                } finally {
                  setFetching(false);
                }
              })();
            }}
          >
            {fetching ? <SpinnerIcon className="h-3.5 w-3.5" /> : <RefreshIcon className="h-3.5 w-3.5" />}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-pressed={editing}
            aria-label={t('editConnection')}
            title={t('editConnection')}
            onClick={() => setEditing((current) => !current)}
          >
            <EditIcon className="h-3.5 w-3.5" />
          </Button>
          {!isLocal && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              iconOnly
              aria-label={t('removeProvider')}
              title={t('removeProviderTitle')}
              onClick={() => onRemove(index)}
            >
              <TrashIcon className="h-3.5 w-3.5" />
            </Button>
          )}
        </>
      }
    >
      {showConnection && (
        <div className="grid gap-3 p-4 sm:grid-cols-2 border-b border-border">
          <FieldRow label={t('nameLabel')}>
            <TextInput
              value={provider.name}
              onChange={(event) => onChange(index, { name: event.target.value })}
              className="w-full"
            />
          </FieldRow>

          <FieldRow
            label={t('baseUrlLabel')}
            hint={isLocal ? t('baseUrlLocalHint') : undefined}
          >
            <TextInput
              value={provider.baseUrl}
              placeholder={isLocal ? listing?.baseUrl ?? '' : ''}
              onChange={(event) => onChange(index, { baseUrl: event.target.value })}
              className="w-full"
            />
          </FieldRow>

          <FieldRow label={t('apiKeyLabel')}>
            <Select
              value={provider.keyKind}
              onChange={(event) =>
                onChange(index, { keyKind: event.target.value as ProviderForm['keyKind'] })
              }
              className="w-full"
            >
              <option value="none">{t('apiKeyNone')}</option>
              <option value="env">{t('apiKeyEnv')}</option>
              <option value="stored">{t('apiKeyStored')}</option>
            </Select>
          </FieldRow>

          {provider.keyKind === 'env' && (
            <FieldRow label={t('apiKeyEnv')}>
              <TextInput
                value={provider.envVar}
                placeholder="OPENROUTER_API_KEY"
                onChange={(event) => onChange(index, { envVar: event.target.value })}
                className="w-full"
              />
            </FieldRow>
          )}

          {provider.keyKind === 'stored' && (
            <FieldRow label={t('newKeyLabel')}>
              <TextInput
                type="password"
                autoComplete="off"
                value={provider.newKey}
                placeholder={provider.sealed ? t('sealedKeyPlaceholder') : t('pasteKeyPlaceholder')}
                onChange={(event) => onChange(index, { newKey: event.target.value })}
                className="w-full"
              />
            </FieldRow>
          )}
        </div>
      )}

      {addingModel && (
        <div className="flex gap-2 px-4 py-2.5 border-b border-border">
          <TextInput
            autoFocus
            value={modelDraft}
            onChange={(event) => setModelDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key !== 'Enter') return;
              event.preventDefault();
              event.stopPropagation();
              addModel();
            }}
            placeholder={t('modelIdPlaceholder')}
            className="w-full max-w-xs"
          />
          <Button
            type="button"
            variant="secondary"
            size="sm"
            iconOnly
            disabled={modelDraft.trim() === ''}
            aria-label={t('add')}
            title={t('addModelBtnTitle')}
            onClick={addModel}
          >
            <PlusIcon className="h-3.5 w-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-label={t('cancel')}
            title={t('cancel')}
            onClick={() => setAddingModel(false)}
          >
            <CloseIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
      )}

      {listing?.models.length === 0 && added.length === 0 ? (
        <p className="m-0 px-4 py-3 text-xs text-faint">{t('noModelsListed')}</p>
      ) : (
        <div className="flex flex-col">
          {listing?.models.map((model) => (
            <ModelRow
              key={`${provider.id}/${model.id}`}
              providerId={provider.id}
              modelId={model.id}
              model={model}
              values={values}
              billing={billing}
              defaults={defaults}
              favorites={favorites}
              isDefault={defaultModel === `${provider.id}/${model.id}`}
              onSetDefault={onSetDefault}
              onChangeModel={onChangeModel}
              onRemoveModel={onRemoveModel}
              onToggleFavorite={onToggleFavorite}
              onToggleBilled={onToggleBilled}
            />
          ))}
          {added.map((modelId) => (
            <ModelRow
              key={`${provider.id}/${modelId}`}
              providerId={provider.id}
              modelId={modelId}
              model={null}
              values={values}
              billing={billing}
              defaults={defaults}
              favorites={favorites}
              isDefault={defaultModel === `${provider.id}/${modelId}`}
              onSetDefault={onSetDefault}
              onChangeModel={onChangeModel}
              onRemoveModel={onRemoveModel}
              onToggleFavorite={onToggleFavorite}
              onToggleBilled={onToggleBilled}
            />
          ))}
        </div>
      )}
    </Section>
  );
}
