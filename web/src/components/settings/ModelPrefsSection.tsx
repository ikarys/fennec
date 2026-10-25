import type { ReasoningEffort, SettingsDefaults, SettingsModel } from '../../lib/api';

export interface ModelForm {
  maxTokens: string;
  contextWindow: string;
  compactionThreshold: string;
  reasoningEffort: string;
}

interface ModelPrefsSectionProps {
  models: SettingsModel[];
  values: Record<string, ModelForm>;
  defaults: SettingsDefaults;
  onChange: (modelId: string, field: keyof ModelForm, value: string) => void;
}

const REASONING_EFFORTS: ReasoningEffort[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

export function percentFromFraction(fraction: number): string {
  return String(Number((fraction * 100).toFixed(2)));
}

export function ModelPrefsSection({ models, values, defaults, onChange }: ModelPrefsSectionProps) {
  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[15px] font-semibold">Paramètres par modèle</h2>
      {models.length === 0 && <p className="mt-3 text-sm text-muted">Aucun modèle listé.</p>}
      {models.map((model) => {
        const value = values[model.id] ?? { maxTokens: '', contextWindow: '', compactionThreshold: '', reasoningEffort: '' };
        return (
          <div key={model.id} className="mt-4 rounded-lg border border-border bg-raised px-4 py-3">
            <h3 className="font-mono text-[13px] font-medium">{model.id}</h3>
            <div className="mt-3 grid gap-3 md:grid-cols-2">
              <label className="block text-[13px] text-muted">
                Tokens max par réponse
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={value.maxTokens}
                  placeholder={String(defaults.maxTokens)}
                  onChange={(event) => onChange(model.id, 'maxTokens', event.target.value)}
                  className="mt-1.5 w-full rounded-lg border border-border bg-panel px-3 py-2 text-[13px] text-text"
                />
              </label>
              <label className="block text-[13px] text-muted">
                Fenêtre de contexte
                <input
                  type="number"
                  min={1024}
                  step={1}
                  value={value.contextWindow}
                  placeholder={String(model.contextWindow ?? defaults.contextWindow)}
                  onChange={(event) => onChange(model.id, 'contextWindow', event.target.value)}
                  className="mt-1.5 w-full rounded-lg border border-border bg-panel px-3 py-2 text-[13px] text-text"
                />
              </label>
              <label className="block text-[13px] text-muted">
                Compaction à (%)
                <input
                  type="number"
                  min={0}
                  max={100}
                  step={1}
                  value={value.compactionThreshold}
                  placeholder={percentFromFraction(defaults.compactAt)}
                  onChange={(event) => onChange(model.id, 'compactionThreshold', event.target.value)}
                  className="mt-1.5 w-full rounded-lg border border-border bg-panel px-3 py-2 text-[13px] text-text"
                />
              </label>
              <label className="block text-[13px] text-muted">
                Effort de réflexion
                <select
                  value={value.reasoningEffort}
                  onChange={(event) => onChange(model.id, 'reasoningEffort', event.target.value)}
                  className="mt-1.5 w-full rounded-lg border border-border bg-panel px-3 py-2 text-[13px] text-text"
                >
                  <option value="">Auto (sans effort)</option>
                  {REASONING_EFFORTS.map((effort) => (
                    <option key={effort} value={effort}>
                      {effort}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <p className="mt-2 text-xs text-muted">0 désactive la compaction automatique.</p>
          </div>
        );
      })}
    </section>
  );
}
