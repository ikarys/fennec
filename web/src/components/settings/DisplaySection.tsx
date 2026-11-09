import type { DisplayPrefs, EditorId } from '../../lib/api';
import { EDITORS } from '../../lib/editors';
import { Segmented } from '../ui/Segmented';
import { Section, type SectionSave } from '../ui/Section';
import { Select } from '../ui/Field';
import { Switch } from '../ui/Switch';
import { themes, type ThemeId } from '../../lib/theme';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

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
const LANGUAGES: { value: 'en' | 'fr'; label: MsgKey }[] = [
  { value: 'en', label: 'displayLanguageEnglish' },
  { value: 'fr', label: 'displayLanguageFrench' }
];

export function DisplaySection({ value, onChange, theme, onThemeChange, save }: DisplaySectionProps) {
  const t = useT();
  return (
    <Section title={t('displayTitle')} save={save}>
      <div className="grid grid-cols-[minmax(7rem,max-content)_1fr] items-center justify-items-start gap-x-6 gap-y-3 text-sm text-text">
        <span className="text-muted">{t('theme')}</span>
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
        <span className="text-muted">{t('displayLanguage')}</span>
        <Segmented
          label={t('displayLanguage')}
          value={value.language ?? 'en'}
          onChange={(language) => onChange({ ...value, language })}
          segments={LANGUAGES.map((language) => ({ value: language.value, label: t(language.label) }))}
        />
        <span className="text-muted">{t('fontSize')}</span>
        <Segmented
          label={t('fontSize')}
          value={value.fontSize}
          onChange={(size) => onChange({ ...value, fontSize: size })}
          segments={FONT_SIZES.map((size) => ({ value: size, label: size }))}
        />
        <span className="self-start pt-1.5 text-muted">{t('editorLabel')}</span>
        <div className="flex flex-col gap-1">
          <Select
            aria-label={t('editorLabel')}
            className="w-48"
            value={value.editor ?? 'vscode'}
            onChange={(event) => onChange({ ...value, editor: event.target.value as EditorId })}
          >
            {EDITORS.map((editor) => (
              <option key={editor.value} value={editor.value}>
                {editor.label}
              </option>
            ))}
          </Select>
          <p className="m-0 text-xs text-faint">{t('editorHint')}</p>
        </div>
        <span className="text-muted">{t('reasoningOpen')}</span>
        <Switch
          label={t('reasoningOpen')}
          checked={value.reasoningOpen}
          onChange={(reasoningOpen) => onChange({ ...value, reasoningOpen })}
        />
        <span className="text-muted">{t('toolResultsOpen')}</span>
        <Switch
          label={t('toolResultsOpen')}
          checked={value.toolResultsOpen}
          onChange={(toolResultsOpen) => onChange({ ...value, toolResultsOpen })}
        />
      </div>
    </Section>
  );
}
