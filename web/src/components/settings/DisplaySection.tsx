import type { DisplayPrefs } from '../../lib/api';
import { Segmented } from '../ui/Segmented';
import { Section } from '../ui/Section';
import { themes, type ThemeId } from '../../lib/theme';

interface DisplaySectionProps {
  value: DisplayPrefs;
  onChange: (value: DisplayPrefs) => void;
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
}

const FONT_SIZES = [13, 14, 15, 16];

export function DisplaySection({ value, onChange, theme, onThemeChange }: DisplaySectionProps) {
  return (
    <Section>
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Affichage</h2>
        <div className="flex items-center gap-3 text-sm text-text">
          <span>Thème</span>
          <Segmented
            label="Thème"
            value={theme}
            onChange={onThemeChange}
            segments={themes.map((option) => ({
              value: option.id,
              ariaLabel: `Thème ${option.label}`,
              label: (
                <>
                  <span className="h-3 w-3 rounded-full border border-border" style={{ background: option.swatch }} />
                  {option.label}
                </>
              )
            }))}
          />
        </div>
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
    </Section>
  );
}
