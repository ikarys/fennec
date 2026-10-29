import type { ModelRef, ParamSource, ProviderListing, ReasoningEffort, SettingsDefaults } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { FieldRow, Select, TextInput } from '../ui/Field';

export interface ModelForm {
  maxTokens: string;
  contextWindow: string;
  compactionThreshold: string;
  reasoningEffort: string;
}

interface ModelPrefsSectionProps {
  listings: ProviderListing[];
  values: Record<string, ModelForm>;
  defaults: SettingsDefaults;
  favorites: ModelRef[];
  onToggleFavorite: (ref: ModelRef) => void;
  onChange: (key: string, field: keyof ModelForm, value: string) => void;
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
      <span className="font-mono">{value}</span>
      <span className="text-faint">({SOURCE_LABELS[source]})</span>
      <span className={`ml-auto ${badge === 'envoyé' ? 'text-ok' : 'text-faint'}`}>{badge}</span>
    </div>
  );
}

export function percentFromFraction(fraction: number): string {
  return String(Number((fraction * 100).toFixed(2)));
}

export function ModelPrefsSection({ listings, values, defaults, favorites, onToggleFavorite, onChange }: ModelPrefsSectionProps) {
  return (
    <Card className="p-4">
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Paramètres par modèle</h2>
        {listings.length === 0 && <p className="text-sm text-muted">Aucun modèle listé.</p>}
        {listings.map((listing) => (
          <div key={listing.provider} className="flex flex-col gap-4 border-t border-border pt-4">
            <h3 className="font-mono text-sm font-medium">{listing.name}</h3>
            {listing.models.length === 0 && <p className="text-sm text-muted">Aucun modèle listé.</p>}
            {listing.models.map((model) => {
              const key = `${listing.provider}/${model.id}`;
              const value =
                values[key] ?? { maxTokens: '', contextWindow: '', compactionThreshold: '', reasoningEffort: '' };
              const isFavorite = favorites.some(
                (favorite) => favorite.provider === listing.provider && favorite.model === model.id
              );
              return (
                <div key={key} className="flex flex-col gap-4">
                  <div className="flex items-center gap-2">
                    <h3 className="min-w-0 truncate font-mono text-sm font-medium">{model.id}</h3>
                    <Button
                      variant="ghost"
                      size="sm"
                      iconOnly
                      aria-label="Afficher dans le composer"
                      title={isFavorite ? 'Retirer des favoris' : 'Afficher dans le composer'}
                      onClick={() => onToggleFavorite({ provider: listing.provider, model: model.id })}
                      className="ml-auto"
                    >
                      {isFavorite ? '★' : '☆'}
                    </Button>
                  </div>
                  <div className="grid gap-3 md:grid-cols-2">
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
                        placeholder={String(model.contextWindow ?? defaults.contextWindow)}
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

                  <div className="flex flex-col gap-1.5 rounded-lg bg-raised/40 p-2.5">
                    <div>
                      <p className="m-0 text-[0.6875rem] font-medium uppercase tracking-wide text-faint">
                        Envoyé au modèle
                      </p>
                      <p className="m-0 text-[0.625rem] text-faint">
                        Valeurs enregistrées — enregistrer pour appliquer les modifications ci-dessus.
                      </p>
                    </div>
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
                      badge={model.resolved.reasoningEffort.value ? 'envoyé' : 'non envoyé'}
                    />
                    {model.resolved.contextWindow.source === 'listed' && (
                      <Button
                        variant="secondary"
                        size="sm"
                        className="mt-1 self-start"
                        onClick={() => onChange(key, 'contextWindow', String(model.resolved.contextWindow.value))}
                      >
                        Figer la fenêtre de contexte
                      </Button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </Card>
  );
}
