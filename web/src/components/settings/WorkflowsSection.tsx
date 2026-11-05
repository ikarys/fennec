import type { WorkflowOptIn } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
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
  function toggle(id: string, checked: boolean) {
    onChange(checked ? [...value, id] : value.filter((workflowId) => workflowId !== id));
  }

  return (
    <Section
      title={t('optionalWorkflowsTitle')}
      description={t('optionalWorkflowsHint')}
      save={save}
    >
      {workflows.length === 0 ? (
        <p className="text-xs text-muted">{t('noOptionalWorkflows')}</p>
      ) : (
        <div className="flex flex-col gap-1">
          {workflows.map((workflow) => (
            <div key={workflow.id} className="flex flex-col gap-1 rounded-md px-2 transition-colors hover:bg-accent-soft -mx-2">
              <label className="flex items-center gap-2 text-sm text-text">
                <input
                  type="checkbox"
                  checked={value.includes(workflow.id)}
                  onChange={(event) => toggle(workflow.id, event.target.checked)}
                  className="h-4 w-4 accent-[var(--accent)]"
                />
                <span>{workflow.name}</span>
              </label>
              <p className="text-xs text-muted">{workflow.description}</p>
            </div>
          ))}
        </div>
      )}
    </Section>
  );
}
