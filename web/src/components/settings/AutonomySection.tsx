// Autonomy night-mode settings section.
import type { AutonomyPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { TextInput } from '../ui/Field';
import { SettingRow } from '../ui/SettingRow';
import { MoonIcon } from '../ui/Icons';
import { useT } from '../../i18n';

interface AutonomySectionProps {
  value: AutonomyPrefs;
  onChange: (value: AutonomyPrefs) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function AutonomySection({ value, onChange, save }: AutonomySectionProps) {
  const t = useT();
  return (
    <Section
      Icon={MoonIcon}
      flush
      title={t('nightModeTitle')}
      description={t('nightModeHint')}
      save={save}
    >
      <SettingRow label={t('maxMinutesPerIdeaLabel')} hint={t('maxMinutesPerIdeaHint')}>
        <TextInput
          type="number"
          min={0}
          step={1}
          className="w-32"
          aria-label={t('maxMinutesPerIdeaLabel')}
          value={value.maxMinutesPerIdea}
          onChange={(event) =>
            onChange({
              ...value,
              maxMinutesPerIdea: Math.max(0, Math.floor(Number(event.target.value) || 0)),
            })
          }
        />
      </SettingRow>

      <SettingRow label={t('maxTokensPerIdeaLabel')} hint={t('maxTokensPerIdeaHint')}>
        <TextInput
          type="number"
          min={0}
          step={1}
          className="w-32"
          aria-label={t('maxTokensPerIdeaLabel')}
          value={value.maxTokensPerIdea}
          onChange={(event) =>
            onChange({
              ...value,
              maxTokensPerIdea: Math.max(0, Math.floor(Number(event.target.value) || 0)),
            })
          }
        />
      </SettingRow>
    </Section>
  );
}
