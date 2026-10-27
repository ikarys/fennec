import type { SettingsModel } from '../../lib/api';

interface ModelSectionProps {
  value: string;
  models: SettingsModel[];
  storedModel: string | null;
  defaultModel: string;
  onChange: (value: string) => void;
}

export function ModelSection({ value, models, storedModel, defaultModel, onChange }: ModelSectionProps) {
  const listedIds = new Set(models.map((model) => model.id));
  const unlistedStored = storedModel && !listedIds.has(storedModel) ? storedModel : null;

  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[0.9375rem] font-semibold">Modèle</h2>
      <label className="mt-3 block text-[0.8125rem] text-muted">
        Modèle
        <select
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="mt-1.5 w-full rounded-lg border border-border bg-raised px-3 py-2 text-[0.8125rem] text-text"
        >
          <option value="">Défaut de l'environnement ({defaultModel})</option>
          {models.map((model) => (
            <option key={model.id} value={model.id}>
              {model.id}
            </option>
          ))}
          {unlistedStored && (
            <option value={unlistedStored}>{unlistedStored} (non listé)</option>
          )}
        </select>
      </label>
    </section>
  );
}
