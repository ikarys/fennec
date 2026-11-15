import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { Switch } from '../ui/Switch';
import { useT } from '../../i18n';

interface WorkflowsSectionProps {
  alwaysValidateTests: boolean;
  onAlwaysValidateTestsChange: (value: boolean) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function WorkflowsSection({ alwaysValidateTests, onAlwaysValidateTestsChange, save }: WorkflowsSectionProps) {
  const t = useT();

  return (
    <Section title={t('workflowsSettingsTitle')} description={t('workflowsSettingsHint')} save={save}>
      <SettingRow label={t('alwaysValidateTestsLabel')} hint={t('alwaysValidateTestsHint')}>
        <Switch
          checked={alwaysValidateTests}
          onChange={onAlwaysValidateTestsChange}
          label={t('alwaysValidateTestsLabel')}
        />
      </SettingRow>
    </Section>
  );
}
