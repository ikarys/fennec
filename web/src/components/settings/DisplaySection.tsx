/** Appearance settings: theme, language/text, thread defaults and notice sounds. */
import type { DisplayPrefs, EditorId } from '../../lib/api';
import { EDITORS } from '../../lib/editors';
import { Segmented } from '../ui/Segmented';
import { Section, type SectionSave } from '../ui/Section';
import { Select } from '../ui/Field';
import { Switch } from '../ui/Switch';
import { ThemePicker } from '../settings/ThemePicker';
import { SettingRow } from '../ui/SettingRow';
import { BellIcon, ChatIcon, EyeIcon, GlobeIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import type { ThemeId } from '../../lib/theme';

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
    <>
      <Section Icon={EyeIcon} title={t('theme')} flush>
        <ThemePicker value={theme} onChange={onThemeChange} />
      </Section>

      <Section Icon={GlobeIcon} title={t('displayTextTitle')} flush save={save}>
        <SettingRow label={t('displayLanguage')} hint={t('displayLanguageHint')}>
          <Segmented
            label={t('displayLanguage')}
            value={value.language ?? 'en'}
            onChange={(language) => onChange({ ...value, language })}
            segments={LANGUAGES.map((language) => ({ value: language.value, label: t(language.label) }))}
          />
        </SettingRow>
        <SettingRow label={t('fontSize')} hint={t('fontSizeHint')}>
          <Segmented
            label={t('fontSize')}
            value={value.fontSize}
            onChange={(size) => onChange({ ...value, fontSize: size })}
            segments={FONT_SIZES.map((size) => ({ value: size, label: size }))}
          />
        </SettingRow>
        <SettingRow label={t('editorLabel')} hint={t('editorHint')}>
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
        </SettingRow>
      </Section>

      <Section Icon={ChatIcon} title={t('displayThreadTitle')} flush save={save}>
        <SettingRow label={t('reasoningOpen')} hint={t('reasoningOpenHint')}>
          <Switch
            label={t('reasoningOpen')}
            checked={value.reasoningOpen}
            onChange={(reasoningOpen) => onChange({ ...value, reasoningOpen })}
          />
        </SettingRow>
        <SettingRow label={t('toolResultsOpen')} hint={t('toolResultsOpenHint')}>
          <Switch
            label={t('toolResultsOpen')}
            checked={value.toolResultsOpen}
            onChange={(toolResultsOpen) => onChange({ ...value, toolResultsOpen })}
          />
        </SettingRow>
      </Section>

      <Section Icon={BellIcon} title={t('displayNoticesTitle')} flush save={save}>
        <SettingRow label={t('soundsLabel')} hint={t('soundsHint')}>
          <Switch
            label={t('soundsLabel')}
            checked={value.sounds ?? true}
            onChange={(sounds) => onChange({ ...value, sounds })}
          />
        </SettingRow>
      </Section>
    </>
  );
}
