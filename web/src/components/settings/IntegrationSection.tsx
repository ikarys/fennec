// Commit integration settings section.
import type { IntegrationPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { CommitIcon } from '../ui/Icons';
import { Switch } from '../ui/Switch';
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
    <Section Icon={CommitIcon} flush title={t('integrationCommitTitle')} save={save}>
      <SettingRow label={t('modelTrailersLabel')} hint={t('modelTrailersHint')}>
        <Switch
          checked={value.modelTrailers}
          onChange={(checked) => onChange({ ...value, modelTrailers: checked })}
          label={t('modelTrailersLabel')}
        />
      </SettingRow>
    </Section>
  );
}
