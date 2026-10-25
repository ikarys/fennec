import type { Origin, WorkflowDef } from '../../lib/workflow';

interface EditorHeaderProps {
  def: WorkflowDef;
  origin: Origin;
  scope: 'project' | 'global';
  dirty: boolean;
  issueCount: number;
  saving: boolean;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onScopeChange: (scope: 'project' | 'global') => void;
  onSave: () => void;
}

const ORIGIN_LABELS: Record<Origin, string> = {
  project: 'Projet',
  global: 'Global',
  default: 'Par défaut'
};

export function EditorHeader({
  def,
  origin,
  scope,
  dirty,
  issueCount,
  saving,
  onNameChange,
  onDescriptionChange,
  onScopeChange,
  onSave
}: EditorHeaderProps) {
  return (
    <header className="flex shrink-0 items-center gap-3 border-b border-border bg-panel px-4 py-2">
      <input
        type="text"
        value={def.name}
        onChange={(event) => onNameChange(event.target.value)}
        aria-label="Nom du workflow"
        className="w-48 rounded-md border border-border bg-transparent px-2 py-1 text-[15px] font-semibold"
      />
      <input
        type="text"
        value={def.description ?? ''}
        onChange={(event) => onDescriptionChange(event.target.value)}
        placeholder="Description"
        aria-label="Description"
        className="grow rounded-md border border-border bg-transparent px-2 py-1 text-[13px] text-muted"
      />
      <span className="rounded-md bg-raised px-2 py-0.5 text-[11px] text-muted">{ORIGIN_LABELS[origin]}</span>
      <select
        value={scope}
        onChange={(event) => onScopeChange(event.target.value as 'project' | 'global')}
        aria-label="Portée d’enregistrement"
        className="rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
      >
        <option value="project">Projet</option>
        <option value="global">Global</option>
      </select>
      {dirty && <span className="text-[13px] text-accent" title="Modifications non enregistrées">●</span>}
      {issueCount > 0 && <span className="text-[13px] text-err">{issueCount} problème(s)</span>}
      <button
        type="button"
        onClick={onSave}
        disabled={saving}
        className="rounded-md bg-accent px-3 py-1 text-[13px] font-medium text-on-accent disabled:opacity-60"
      >
        {saving ? 'Enregistrement…' : 'Enregistrer'}
      </button>
    </header>
  );
}
