import type { WorkflowOptIn } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { Switch } from '../ui/Switch';
import { OptInList } from './OptInList';
import { useT } from '../../i18n';

interface WorkflowsSectionProps {
  workflows: WorkflowOptIn[];
  value: string[];
  onChange: (value: string[]) => void;
  alwaysValidateTests: boolean;
  onAlwaysValidateTestsChange: (value: boolean) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function WorkflowsSection({
  workflows,
  value,
  onChange,
  alwaysValidateTests,
  onAlwaysValidateTestsChange,
  save,
}: WorkflowsSectionProps) {
  const t = useT();

  return (
    <Section
      title={t('optionalWorkflowsTitle')}
      description={t('optionalWorkflowsHint')}
      save={save}
    >
      <SettingRow label={t('alwaysValidateTestsLabel')} hint={t('alwaysValidateTestsHint')}>
        <Switch
          checked={alwaysValidateTests}
          onChange={onAlwaysValidateTestsChange}
          label={t('alwaysValidateTestsLabel')}
        />
      </SettingRow>
      <OptInList
        title={t('optInWorkflowsTitle')}
        items={workflows.map((workflow) => ({
          id: workflow.id,
          label: workflow.name,
          description: workflow.description,
        }))}
        value={value}
        onChange={onChange}
        empty={t('noOptionalWorkflows')}
      />
    </Section>
  );
}
