import type { DisplayPrefs } from '../../lib/api';
import { Segmented } from '../ui/Segmented';
import { Section, type SectionSave } from '../ui/Section';
import { themes, type ThemeId } from '../../lib/theme';
import { useT } from '../../i18n';

interface DisplaySectionProps {
  value: DisplayPrefs;
  onChange: (value: DisplayPrefs) => void;
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

const FONT_SIZES = [13, 14, 15, 16];

// Language names stay in their own language, in both UIs.
const LANGUAGES: { value: 'en' | 'fr'; label: string }[] = [
  { value: 'en', label: 'English' },
  { value: 'fr', label: 'Français' }
];

export function DisplaySection({ value, onChange, theme, onThemeChange, save }: DisplaySectionProps) {
  const t = useT();
  return (
    <Section title={t('displayTitle')} save={save}>
      <div className="flex items-center gap-3 text-sm text-text">
        <span>{t('theme')}</span>
        <Segmented
          label={t('theme')}
          value={theme}
          onChange={onThemeChange}
          segments={themes.map((option) => ({
            value: option.id,
            ariaLabel: `${t('theme')} ${option.label}`,
            label: option.label
          }))}
        />
      </div>
      <div className="flex items-center gap-3 text-sm text-text">
        <span>{t('displayLanguage')}</span>
        <Segmented
          label={t('displayLanguage')}
          value={value.language ?? 'en'}
          onChange={(language) => onChange({ ...value, language })}
          segments={LANGUAGES.map((language) => ({ value: language.value, label: language.label }))}
        />
      </div>
      <label className="flex items-center gap-2 text-sm text-text">
        <input
          type="checkbox"
          checked={value.reasoningOpen}
          onChange={(event) => onChange({ ...value, reasoningOpen: event.target.checked })}
          className="h-4 w-4 accent-[var(--accent)]"
        />
        {t('reasoningOpen')}
      </label>
      <label className="flex items-center gap-2 text-sm text-text">
        <input
          type="checkbox"
          checked={value.toolResultsOpen}
          onChange={(event) => onChange({ ...value, toolResultsOpen: event.target.checked })}
          className="h-4 w-4 accent-[var(--accent)]"
        />
        {t('toolResultsOpen')}
      </label>
      <div className="flex items-center gap-3 text-sm text-text">
        <span>{t('fontSize')}</span>
        <Segmented
          label={t('fontSize')}
          value={value.fontSize}
          onChange={(size) => onChange({ ...value, fontSize: size })}
          segments={FONT_SIZES.map((size) => ({ value: size, label: size }))}
        />
      </div>
    </Section>
  );
}
