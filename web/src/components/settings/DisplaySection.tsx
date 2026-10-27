import type { DisplayPrefs } from '../../lib/api';

interface DisplaySectionProps {
  value: DisplayPrefs;
  onChange: (value: DisplayPrefs) => void;
}

const FONT_SIZES = [13, 14, 15, 16];

export function DisplaySection({ value, onChange }: DisplaySectionProps) {
  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[0.9375rem] font-semibold">Affichage</h2>
      <label className="mt-3 flex items-center gap-2 text-[0.8125rem] text-text">
        <input
          type="checkbox"
          checked={value.reasoningOpen}
          onChange={(event) => onChange({ ...value, reasoningOpen: event.target.checked })}
          className="h-4 w-4 accent-accent"
        />
        Réflexion ouverte par défaut
      </label>
      <label className="mt-2 flex items-center gap-2 text-[0.8125rem] text-text">
        <input
          type="checkbox"
          checked={value.toolResultsOpen}
          onChange={(event) => onChange({ ...value, toolResultsOpen: event.target.checked })}
          className="h-4 w-4 accent-accent"
        />
        Résultats d'outils ouverts par défaut
      </label>
      <div className="mt-3 flex items-center gap-3 text-[0.8125rem] text-text">
        <span>Taille du texte</span>
        <div className="flex overflow-hidden rounded-lg border border-border">
          {FONT_SIZES.map((size) => {
            const active = value.fontSize === size;
            return (
              <button
                key={size}
                type="button"
                onClick={() => onChange({ ...value, fontSize: size })}
                className={`px-2.5 py-1 text-xs transition-colors ${
                  active ? 'bg-accent text-on-accent' : 'bg-panel text-muted hover:bg-raised hover:text-text'
                }`}
              >
                {size}
              </button>
            );
          })}
        </div>
      </div>
    </section>
  );
}
