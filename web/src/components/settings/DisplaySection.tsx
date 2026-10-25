import type { DisplayPrefs } from '../../lib/api';

interface DisplaySectionProps {
  value: DisplayPrefs;
  onChange: (value: DisplayPrefs) => void;
}

export function DisplaySection({ value, onChange }: DisplaySectionProps) {
  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[15px] font-semibold">Affichage</h2>
      <label className="mt-3 flex items-center gap-2 text-[13px] text-text">
        <input
          type="checkbox"
          checked={value.reasoningOpen}
          onChange={(event) => onChange({ ...value, reasoningOpen: event.target.checked })}
          className="h-4 w-4 accent-accent"
        />
        Réflexion ouverte par défaut
      </label>
      <label className="mt-2 flex items-center gap-2 text-[13px] text-text">
        <input
          type="checkbox"
          checked={value.toolResultsOpen}
          onChange={(event) => onChange({ ...value, toolResultsOpen: event.target.checked })}
          className="h-4 w-4 accent-accent"
        />
        Résultats d'outils ouverts par défaut
      </label>
    </section>
  );
}
