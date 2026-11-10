// One model row: header with spec pills and an expandable parameter/price editor.
import { useState } from 'react';
import type { ModelRef, ParamSource, ProviderListing, ReasoningEffort, SettingsDefaults } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ChevronDownIcon, CloseIcon, HeartIcon, InfoIcon, PinIcon, StarIcon } from '../ui/Icons';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { ModelProbe } from './ModelProbe';
import {
  billedByDefault,
  emptyModelForm,
  modelSpecs,
  percentFromFraction,
  type ModelForm
} from '../../lib/settingsForm';
import { useT } from '../../i18n';
import { type MsgKey } from '../../i18n/messages';

type ListedModel = ProviderListing['models'][number];

// The README section a billed flag's info icon opens.
const BILLING_DOC_URL = 'https://github.com/ikarys/fennec#budgets-and-billed-models';

const REASONING_EFFORTS: ReasoningEffort[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

const SOURCE_LABELS: Record<ParamSource, MsgKey> = {
  perModel: 'sourcePerModel',
  listed: 'sourceListed',
  default: 'sourceDefault'
};

/** One line of the effective-parameters read-out for a model. */
function ResolvedRow({
  label,
  value,
  source,
  badge
}: {
  label: string;
  value: string;
  source: ParamSource;
  badge: MsgKey;
}) {
  const t = useT();
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="w-24 shrink-0 text-muted">{label}</span>
      <span className="font-mono tabular-nums">{value}</span>
      <span className="text-faint">({t(SOURCE_LABELS[source])})</span>
      <span className={`ml-auto ${badge === 'badgeSent' ? 'text-ok' : 'text-faint'}`}>{t(badge)}</span>
    </div>
  );
}

interface ModelRowProps {
  providerId: string;
  modelId: string;
  /** Null for a hand-added model (no listing entry yet). */
  model: ListedModel | null;
  values: Record<string, ModelForm>;
  billing: Record<string, boolean>;
  defaults: SettingsDefaults;
  favorites: ModelRef[];
  isDefault: boolean;
  onSetDefault: (key: string) => void;
  onChangeModel: (key: string, field: keyof ModelForm, value: string) => void;
  onRemoveModel: (providerId: string, modelId: string) => void;
  onToggleFavorite: (ref: ModelRef) => void;
  onToggleBilled: (key: string, billed: boolean) => void;
}

export function ModelRow({
  providerId,
  modelId,
  model,
  values,
  billing,
  defaults,
  favorites,
  isDefault,
  onSetDefault,
  onChangeModel,
  onRemoveModel,
  onToggleFavorite,
  onToggleBilled
}: ModelRowProps) {
  const t = useT();
  const [open, setOpen] = useState(false);

  const key = `${providerId}/${modelId}`;
  const value = values[key] ?? emptyModelForm;
  const isFavorite = favorites.some((favorite) => favorite.provider === providerId && favorite.model === modelId);
  const autoEffort = t('modelDefaultEffort', { effort: defaults.reasoningEffort ?? '—' });
  const specs = modelSpecs(value, model, autoEffort);
  const displaySpecs = model === null && specs.length === 0 ? [t('modelDefaultParams')] : specs;

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1 px-4 py-2 border-b border-border last:border-b-0 hover:bg-accent-soft/40">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          iconOnly
          aria-pressed={isDefault}
          aria-label={t('defaultModelLabel')}
          title={isDefault ? t('defaultModelOn') : t('defaultModelOff')}
          onClick={() => onSetDefault(isDefault ? '' : key)}
          className={`text-base ${isDefault ? 'text-accent' : 'text-faint'}`}
        >
          <StarIcon className="h-3 w-3" filled={isDefault} />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          iconOnly
          aria-pressed={isFavorite}
          aria-label={t('showInComposer')}
          title={isFavorite ? t('removeFavorite') : t('showInComposer')}
          onClick={() => onToggleFavorite({ provider: providerId, model: modelId })}
          className={`text-base ${isFavorite ? 'text-accent' : 'text-faint'}`}
        >
          <HeartIcon className="h-3 w-3" filled={isFavorite} />
        </Button>

        <div className="min-w-0 grow">
          <div className="flex items-center gap-1.5">
            <span className="font-mono text-[13px] text-text truncate">{modelId}</span>
            {model === null && <Badge tone="neutral">{t('manualBadge')}</Badge>}
          </div>
          <div className="flex flex-wrap gap-1 mt-0.5">
            {displaySpecs.map((spec, index) => (
              <Badge key={`${spec}-${index}`} tone="neutral" appearance="pill">
                {spec}
              </Badge>
            ))}
          </div>
        </div>

        {model === null && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-label={t('removeModel')}
            title={t('removeManualModel')}
            onClick={() => onRemoveModel(providerId, modelId)}
          >
            <CloseIcon className="h-3 w-3" />
          </Button>
        )}

        <Button
          type="button"
          variant="ghost"
          size="sm"
          iconOnly
          aria-expanded={open}
          aria-label={t('modelParams')}
          onClick={() => setOpen((current) => !current)}
        >
          <ChevronDownIcon className={`h-3.5 w-3.5 transition-transform ${open ? 'rotate-180' : ''}`} />
        </Button>
      </div>

      {open && (
        <div className="flex flex-col gap-3 bg-bg/40 px-4 pb-4 pt-3 border-b border-border">
          <ModelProbe
            providerId={providerId}
            modelId={modelId}
            onPatch={(patch) => {
              for (const [field, fieldValue] of Object.entries(patch)) {
                onChangeModel(key, field as keyof ModelForm, fieldValue);
              }
            }}
          />

          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <FieldRow label={t('maxTokensLabel')}>
              <TextInput
                type="number"
                min={1}
                step={1}
                value={value.maxTokens}
                placeholder={String(defaults.maxTokens)}
                onChange={(event) => onChangeModel(key, 'maxTokens', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('contextWindowLabel')}>
              <TextInput
                type="number"
                min={1024}
                step={1}
                value={value.contextWindow}
                placeholder={String(model?.contextWindow ?? defaults.contextWindow)}
                onChange={(event) => onChangeModel(key, 'contextWindow', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('compactionLabel')} hint={t('compactionHint')}>
              <TextInput
                type="number"
                min={0}
                max={100}
                step={1}
                value={value.compactionThreshold}
                placeholder={percentFromFraction(defaults.compactAt)}
                onChange={(event) => onChangeModel(key, 'compactionThreshold', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('effortLabel')}>
              <Select
                value={value.reasoningEffort}
                onChange={(event) => onChangeModel(key, 'reasoningEffort', event.target.value)}
                className="w-full"
              >
                <option value="">{t('effortAuto')}</option>
                {REASONING_EFFORTS.map((effort) => (
                  <option key={effort} value={effort}>
                    {effort}
                  </option>
                ))}
              </Select>
            </FieldRow>

            <p className="m-0 text-xs font-medium text-muted sm:col-span-2 xl:col-span-4">
              {t('priceHeader')}
            </p>
            <FieldRow label={t('inputLabel')}>
              <TextInput
                type="number"
                min={0}
                step="any"
                value={value.priceInput}
                placeholder="—"
                onChange={(event) => onChangeModel(key, 'priceInput', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('cacheLabel')}>
              <TextInput
                type="number"
                min={0}
                step="any"
                value={value.priceCached}
                placeholder="—"
                onChange={(event) => onChangeModel(key, 'priceCached', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label={t('outputLabel')}>
              <TextInput
                type="number"
                min={0}
                step="any"
                value={value.priceOutput}
                placeholder="—"
                onChange={(event) => onChangeModel(key, 'priceOutput', event.target.value)}
                className="w-full"
              />
            </FieldRow>

            <div className="sm:col-span-2 xl:col-span-4 flex items-center gap-1.5 text-sm">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={billing[key] ?? billedByDefault(providerId)}
                  onChange={(event) => onToggleBilled(key, event.target.checked)}
                />
                {t('billedLabel')}
              </label>
              <a
                href={BILLING_DOC_URL}
                target="_blank"
                rel="noreferrer"
                title={t('billedHint')}
                aria-label={t('billedHint')}
                className="text-faint hover:text-accent"
              >
                <InfoIcon className="h-3.5 w-3.5" />
              </a>
            </div>
          </div>

          {model !== null && (
            <div className="flex flex-col gap-1.5 border-l border-border pl-3">
              <p className="m-0 text-xs font-medium text-muted">
                {t('sentToModel')}{' '}
                <span className="font-normal text-faint">
                  {t('sentToModelSuffix')}
                </span>
              </p>
              <ResolvedRow
                label={t('resolvedMaxTokens')}
                value={String(model.resolved.maxTokens.value)}
                source={model.resolved.maxTokens.source}
                badge={model.resolved.maxTokens.wire ? 'badgeSent' : 'badgeLocal'}
              />
              <ResolvedRow
                label={t('resolvedContext')}
                value={String(model.resolved.contextWindow.value)}
                source={model.resolved.contextWindow.source}
                badge="badgeLocal"
              />
              <ResolvedRow
                label={t('resolvedCompaction')}
                value={`${percentFromFraction(model.resolved.compactAt.value)} %`}
                source={model.resolved.compactAt.source}
                badge="badgeLocal"
              />
              <ResolvedRow
                label={t('resolvedEffort')}
                value={model.resolved.reasoningEffort.value ?? '—'}
                source={model.resolved.reasoningEffort.source}
                badge={model.resolved.reasoningEffort.wire ? 'badgeSent' : 'badgeNotSent'}
              />
              {model.resolved.contextWindow.source === 'listed' && (
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  iconOnly
                  className="mt-1 self-start"
                  aria-label={t('pin')}
                  title={t('pinContext')}
                  onClick={() => onChangeModel(key, 'contextWindow', String(model.resolved.contextWindow.value))}
                >
                  <PinIcon className="h-3 w-3" />
                </Button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
