import type { WorkflowOptIn } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { OptInList } from './OptInList';
import { useT } from '../../i18n';

interface WorkflowsSectionProps {
  workflows: WorkflowOptIn[];
  value: string[];
  onChange: (value: string[]) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function WorkflowsSection({ workflows, value, onChange, save }: WorkflowsSectionProps) {
  const t = useT();

  return (
    <Section
      title={t('optionalWorkflowsTitle')}
      description={t('optionalWorkflowsHint')}
      save={save}
    >
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
