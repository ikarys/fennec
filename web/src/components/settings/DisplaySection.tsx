import type { DisplayPrefs } from '../../lib/api';
import { Segmented } from '../ui/Segmented';
import { Card } from '../ui/Card';

interface DisplaySectionProps {
  value: DisplayPrefs;
  onChange: (value: DisplayPrefs) => void;
}

const FONT_SIZES = [13, 14, 15, 16];

export function DisplaySection({ value, onChange }: DisplaySectionProps) {
  return (
    <Card className="p-4">
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Affichage</h2>
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={value.reasoningOpen}
            onChange={(event) => onChange({ ...value, reasoningOpen: event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Réflexion ouverte par défaut
        </label>
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={value.toolResultsOpen}
            onChange={(event) => onChange({ ...value, toolResultsOpen: event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Résultats d'outils ouverts par défaut
        </label>
        <div className="flex items-center gap-3 text-sm text-text">
          <span>Taille du texte</span>
          <Segmented
            label="Taille du texte"
            value={value.fontSize}
            onChange={(size) => onChange({ ...value, fontSize: size })}
            segments={FONT_SIZES.map((size) => ({ value: size, label: size }))}
          />
        </div>
      </div>
    </Card>
  );
}
