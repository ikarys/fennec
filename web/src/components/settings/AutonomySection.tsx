import type { AutonomyPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';
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
    <Section title={t('nightModeTitle')} save={save}>
      <p className="text-xs text-muted">{t('nightModeHint')}</p>

      <div className="grid gap-2 md:grid-cols-2">
        <FieldRow label={t('maxMinutesPerIdeaLabel')} hint={t('maxMinutesPerIdeaHint')}>
          <TextInput
            type="number"
            min={0}
            step={1}
            className="w-full"
            value={value.maxMinutesPerIdea}
            onChange={(event) =>
              onChange({
                ...value,
                maxMinutesPerIdea: Math.max(0, Math.floor(Number(event.target.value) || 0)),
              })
            }
          />
        </FieldRow>

        <FieldRow label={t('maxTokensPerIdeaLabel')} hint={t('maxTokensPerIdeaHint')}>
          <TextInput
            type="number"
            min={0}
            step={1}
            className="w-full"
            value={value.maxTokensPerIdea}
            onChange={(event) =>
              onChange({
                ...value,
                maxTokensPerIdea: Math.max(0, Math.floor(Number(event.target.value) || 0)),
              })
            }
          />
        </FieldRow>
      </div>
    </Section>
  );
}
