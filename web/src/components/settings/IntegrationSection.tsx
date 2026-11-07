import type { IntegrationPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { useT } from '../../i18n';

interface IntegrationSectionProps {
  value: IntegrationPrefs;
  onChange: (value: IntegrationPrefs) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function IntegrationSection({ value, onChange, save }: IntegrationSectionProps) {
  const t = useT();
  return (
    <Section title={t('integrationCommitTitle')} save={save}>
      <div className="flex flex-col gap-1">
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={value.modelTrailers}
            onChange={(event) => onChange({ ...value, modelTrailers: event.target.checked })}
            className="h-4 w-4"
          />
          {t('modelTrailersLabel')}
        </label>
        <p className="text-xs text-muted">{t('modelTrailersHint')}</p>
      </div>
    </Section>
  );
}
