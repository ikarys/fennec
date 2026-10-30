import type { WorkflowOptIn } from '../../lib/api';
import { Section } from '../ui/Section';

interface WorkflowsSectionProps {
  workflows: WorkflowOptIn[];
  value: string[];
  onChange: (value: string[]) => void;
}

export function WorkflowsSection({ workflows, value, onChange }: WorkflowsSectionProps) {
  function toggle(id: string, checked: boolean) {
    onChange(checked ? [...value, id] : value.filter((workflowId) => workflowId !== id));
  }

  return (
    <Section
      title="Workflows optionnels"
      description="Embarqués dans fennec, désactivés par défaut. Cochés, ils apparaissent dans la liste des workflows de chaque projet."
    >
      {workflows.length === 0 ? (
        <p className="text-xs text-muted">Aucun workflow optionnel.</p>
      ) : (
        <div className="flex flex-col gap-1">
          {workflows.map((workflow) => (
            <div key={workflow.id} className="flex flex-col gap-1">
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
