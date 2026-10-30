import { useState } from 'react';
import type { ModelRef, ParamSource, ProviderListing, ReasoningEffort, SettingsDefaults } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Section } from '../ui/Section';
import { SectionLabel } from '../ui/SectionLabel';
import { ChevronRightIcon, CloseIcon, HeartIcon, PinIcon, StarIcon } from '../Icons';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { ModelProbe } from './ModelProbe';

export interface ModelForm {
  maxTokens: string;
  contextWindow: string;
  compactionThreshold: string;
  reasoningEffort: string;
  priceInput: string;
  priceCached: string;
  priceOutput: string;
}

interface ModelPrefsSectionProps {
  listings: ProviderListing[];
  values: Record<string, ModelForm>;
  defaults: SettingsDefaults;
  favorites: ModelRef[];
  onToggleFavorite: (ref: ModelRef) => void;
  /** `provider/model` of the default model, '' when none is chosen. */
  defaultModel: string;
  onSetDefault: (key: string) => void;
  onChange: (key: string, field: keyof ModelForm, value: string) => void;
  onRemoveModel: (providerId: string, modelId: string) => void;
}

/** The model ids of `provider` edited in the form but absent from its listing: added by hand. */
export function formOnlyModels(values: Record<string, ModelForm>, providerId: string, listing: ProviderListing | undefined): string[] {
  const listedIds = new Set(listing?.models.map((model) => model.id) ?? []);
  const prefix = `${providerId}/`;
  return Object.keys(values)
    .filter((key) => key.startsWith(prefix))
    .map((key) => key.slice(prefix.length))
    .filter((modelId) => !listedIds.has(modelId))
    .sort();
}

const REASONING_EFFORTS: ReasoningEffort[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

const SOURCE_LABELS: Record<ParamSource, string> = {
  perModel: 'par modèle',
  listed: 'listing',
  default: 'défaut'
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
  badge: string;
}) {
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="w-24 shrink-0 text-muted">{label}</span>
      <span className="font-mono tabular-nums">{value}</span>
      <span className="text-faint">({SOURCE_LABELS[source]})</span>
      <span className={`ml-auto ${badge === 'envoyé' ? 'text-ok' : 'text-faint'}`}>{badge}</span>
    </div>
  );
}

export function percentFromFraction(fraction: number): string {
  return String(Number((fraction * 100).toFixed(2)));
}

/** The compact line of a model row: what its turn runs with, at a glance. */
function priceSuffix(base: string, value: ModelForm): string {
  if (value.priceInput === '' && value.priceOutput === '') return base;
  return `${base} · $${value.priceInput || '0'}/$${value.priceOutput || '0'}`;
}

function summary(value: ModelForm, model: ProviderListing['models'][number] | null, defaults: SettingsDefaults): string {
  const parts: string[] = [];
  if (model === null) {
    // A hand-added model: nothing is resolved until it is saved and used.
    const base = value.contextWindow !== '' ? `ctx ${value.contextWindow}` : 'paramètres par défaut';
    return priceSuffix(base, value);
  }
  const window = value.contextWindow !== '' ? value.contextWindow : String(model.resolved.contextWindow.value);
  parts.push(`ctx ${window}`);
  const max = value.maxTokens !== '' ? value.maxTokens : String(model.resolved.maxTokens.value);
  parts.push(`max ${max}`);
  const effort =
    value.reasoningEffort !== ''
      ? value.reasoningEffort
      : (model.resolved.reasoningEffort.value ?? `défaut ${defaults.reasoningEffort ?? '—'}`);
  parts.push(`effort ${effort}`);
  return priceSuffix(parts.join(' · '), value);
}

/** One model row: from the listing, or a hand-added one from the form only. */
function ModelRow({
  providerId,
  modelId,
  model,
  values,
  defaults,
  favorites,
  onToggleFavorite,
  isDefault,
  onSetDefault,
  onChange,
  onRemoveModel,
  open,
  onToggleOpen
}: {
  providerId: string;
  modelId: string;
  /** Null for a hand-added model (no listing entry yet). */
  model: ProviderListing['models'][number] | null;
  values: Record<string, ModelForm>;
  defaults: SettingsDefaults;
  favorites: ModelRef[];
  onToggleFavorite: (ref: ModelRef) => void;
  isDefault: boolean;
  onSetDefault: (key: string) => void;
  onChange: (key: string, field: keyof ModelForm, value: string) => void;
  onRemoveModel: (providerId: string, modelId: string) => void;
  open: boolean;
  onToggleOpen: () => void;
}) {
  const key = `${providerId}/${modelId}`;
  const value = values[key] ?? { maxTokens: '', contextWindow: '', compactionThreshold: '', reasoningEffort: '', priceInput: '', priceCached: '', priceOutput: '' };
  const isFavorite = favorites.some((favorite) => favorite.provider === providerId && favorite.model === modelId);

  return (
    <div className="flex flex-col border-t border-border/60 first:border-t-0">
      <div className="-mx-2 flex h-8 items-center gap-2 rounded px-2 hover:bg-raised">
        <button
          type="button"
          aria-expanded={open}
          onClick={onToggleOpen}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <ChevronRightIcon className={`h-3 w-3 shrink-0 text-muted transition-transform ${open ? 'rotate-90' : ''}`} />
          <span className="min-w-0 truncate font-mono text-xs">{modelId}</span>
          {model === null && <Badge tone="neutral">manuel</Badge>}
          <span className="shrink-0 text-xs text-faint tabular-nums">{summary(value, model, defaults)}</span>
        </button>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-pressed={isDefault}
          aria-label="Modèle par défaut"
          title={isDefault ? 'Modèle par défaut des nouvelles sessions (cliquer pour retirer)' : 'Utiliser par défaut dans les nouvelles sessions'}
          onClick={() => onSetDefault(isDefault ? '' : key)}
          className={`text-base ${isDefault ? 'text-accent' : 'text-faint'}`}
        >
          <StarIcon className="h-3 w-3" filled={isDefault} />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-pressed={isFavorite}
          aria-label="Afficher dans le composer"
          title={isFavorite ? 'Retirer des favoris' : 'Afficher dans le composer'}
          onClick={() => onToggleFavorite({ provider: providerId, model: modelId })}
          className={`text-base ${isFavorite ? 'text-accent' : 'text-faint'}`}
        >
          <HeartIcon className="h-3 w-3" filled={isFavorite} />
        </Button>
        {model === null && (
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            aria-label="Retirer le modèle"
            title="Retirer ce modèle manuel"
            onClick={() => onRemoveModel(providerId, modelId)}
          >
            <CloseIcon className="h-3 w-3" />
          </Button>
        )}
      </div>

      {open && (
        <div className="flex flex-col gap-3 pb-3 pl-5">
          <ModelProbe
            providerId={providerId}
            modelId={modelId}
            onPatch={(patch) => {
              for (const [field, fieldValue] of Object.entries(patch)) {
                onChange(key, field as keyof ModelForm, fieldValue);
              }
            }}
          />
          <div className="grid gap-2 md:grid-cols-2">
            <FieldRow label="Tokens max par réponse">
              <TextInput
                type="number"
                min={1}
                step={1}
                value={value.maxTokens}
                placeholder={String(defaults.maxTokens)}
                onChange={(event) => onChange(key, 'maxTokens', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="Fenêtre de contexte">
              <TextInput
                type="number"
                min={1024}
                step={1}
                value={value.contextWindow}
                placeholder={String(model?.contextWindow ?? defaults.contextWindow)}
                onChange={(event) => onChange(key, 'contextWindow', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="Compaction à (%)" hint="0 désactive la compaction automatique.">
              <TextInput
                type="number"
                min={0}
                max={100}
                step={1}
                value={value.compactionThreshold}
                placeholder={percentFromFraction(defaults.compactAt)}
                onChange={(event) => onChange(key, 'compactionThreshold', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="Effort de réflexion">
              <Select
                value={value.reasoningEffort}
                onChange={(event) => onChange(key, 'reasoningEffort', event.target.value)}
                className="w-full"
              >
                <option value="">Auto (sans effort)</option>
                {REASONING_EFFORTS.map((effort) => (
                  <option key={effort} value={effort}>
                    {effort}
                  </option>
                ))}
              </Select>
            </FieldRow>
          </div>
          <div className="grid gap-2 md:grid-cols-3">
            <p className="m-0 text-xs font-medium text-muted md:col-span-3">
              Prix ($ par million de tokens, pour l'estimation d'Insight)
            </p>
            <FieldRow label="Entrée">
              <TextInput
                type="number"
                min={0}
                step="any"
                value={value.priceInput}
                placeholder="—"
                onChange={(event) => onChange(key, 'priceInput', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="Cache">
              <TextInput
                type="number"
                min={0}
                step="any"
                value={value.priceCached}
                placeholder="—"
                onChange={(event) => onChange(key, 'priceCached', event.target.value)}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="Sortie">
              <TextInput
                type="number"
                min={0}
                step="any"
                value={value.priceOutput}
                placeholder="—"
                onChange={(event) => onChange(key, 'priceOutput', event.target.value)}
                className="w-full"
              />
            </FieldRow>
          </div>

          {model !== null && (
            <div className="flex flex-col gap-1.5 border-l border-border pl-3">
              <p className="m-0 text-xs font-medium text-muted">
                Envoyé au modèle{' '}
                <span className="font-normal text-faint">
                  — valeurs enregistrées, « Enregistrer » pour appliquer
                </span>
              </p>
              <ResolvedRow
                label="Tokens max"
                value={String(model.resolved.maxTokens.value)}
                source={model.resolved.maxTokens.source}
                badge={model.resolved.maxTokens.wire ? 'envoyé' : 'local'}
              />
              <ResolvedRow
                label="Contexte"
                value={String(model.resolved.contextWindow.value)}
                source={model.resolved.contextWindow.source}
                badge="local"
              />
              <ResolvedRow
                label="Compaction"
                value={`${percentFromFraction(model.resolved.compactAt.value)} %`}
                source={model.resolved.compactAt.source}
                badge="local"
              />
              <ResolvedRow
                label="Effort"
                value={model.resolved.reasoningEffort.value ?? '—'}
                source={model.resolved.reasoningEffort.source}
                badge={model.resolved.reasoningEffort.wire ? 'envoyé' : 'non envoyé'}
              />
              {model.resolved.contextWindow.source === 'listed' && (
                <Button
                  variant="secondary"
                  size="sm"
                  iconOnly
                  className="mt-1 self-start"
                  aria-label="Figer"
                  title="Figer la fenêtre de contexte"
                  onClick={() => onChange(key, 'contextWindow', String(model.resolved.contextWindow.value))}
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

export function ModelPrefsSection({
  listings,
  values,
  defaults,
  favorites,
  onToggleFavorite,
  defaultModel,
  onSetDefault,
  onChange,
  onRemoveModel
}: ModelPrefsSectionProps) {
  // A model with saved overrides opens by itself: that is what the human came to edit.
  const hasOverrides = (key: string) => {
    const value = values[key];
    return value != null && (value.maxTokens !== '' || value.contextWindow !== '' || value.compactionThreshold !== '' || value.reasoningEffort !== '' || value.priceInput !== '' || value.priceCached !== '' || value.priceOutput !== '');
  };
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const isOpen = (key: string) => open[key] ?? hasOverrides(key);
  const toggleOpen = (key: string) => setOpen((previous) => ({ ...previous, [key]: !isOpen(key) }));

  return (
    <Section title="Paramètres par modèle">
      {listings.length === 0 && <p className="m-0 text-sm text-muted">Aucun modèle listé.</p>}
      <div className="flex flex-col">
        {listings.map((listing) => {
          const formOnly = formOnlyModels(values, listing.provider, listing);
          const count = listing.models.length + formOnly.length;
          return (
            <div key={listing.provider} className="border-t border-border first:border-t-0 pt-3 first:pt-0">
              <SectionLabel className="pb-1">
                <span className="font-mono">{listing.name}</span>
                {count > 0 && <span className="text-faint"> · {count}</span>}
              </SectionLabel>
              {listing.models.length === 0 && formOnly.length === 0 && (
                <p className="m-0 pb-2 text-xs text-faint">Aucun modèle listé.</p>
              )}
              {listing.models.map((model) => (
                <ModelRow
                  key={`${listing.provider}/${model.id}`}
                  providerId={listing.provider}
                  modelId={model.id}
                  model={model}
                  values={values}
                  defaults={defaults}
                  favorites={favorites}
                  onToggleFavorite={onToggleFavorite}
                  isDefault={defaultModel === `${listing.provider}/${model.id}`}
                  onSetDefault={onSetDefault}
                  onChange={onChange}
                  onRemoveModel={onRemoveModel}
                  open={isOpen(`${listing.provider}/${model.id}`)}
                  onToggleOpen={() => toggleOpen(`${listing.provider}/${model.id}`)}
                />
              ))}
              {formOnly.map((modelId) => (
                <ModelRow
                  key={`${listing.provider}/${modelId}`}
                  providerId={listing.provider}
                  modelId={modelId}
                  model={null}
                  values={values}
                  defaults={defaults}
                  favorites={favorites}
                  onToggleFavorite={onToggleFavorite}
                  isDefault={defaultModel === `${listing.provider}/${modelId}`}
                  onSetDefault={onSetDefault}
                  onChange={onChange}
                  onRemoveModel={onRemoveModel}
                  open={isOpen(`${listing.provider}/${modelId}`)}
                  onToggleOpen={() => toggleOpen(`${listing.provider}/${modelId}`)}
                />
              ))}
            </div>
          );
        })}
      </div>
    </Section>
  );
}
