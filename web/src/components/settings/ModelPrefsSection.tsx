import type { ProviderListing, ReasoningEffort, SettingsDefaults } from '../../lib/api';
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
  onChange: (key: string, field: keyof ModelForm, value: string) => void;
}

const REASONING_EFFORTS: ReasoningEffort[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

export function percentFromFraction(fraction: number): string {
  return String(Number((fraction * 100).toFixed(2)));
}

export function ModelPrefsSection({ listings, values, defaults, onChange }: ModelPrefsSectionProps) {
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
              return (
                <div key={key} className="flex flex-col gap-4">
                  <h3 className="font-mono text-sm font-medium">{model.id}</h3>
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
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </Card>
  );
}
