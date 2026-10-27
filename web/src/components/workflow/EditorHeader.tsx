import type { Origin, WorkflowDef } from '../../lib/workflow';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Select, TextInput } from '../ui/Field';

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
  onTest: (() => void) | null;
  canTest: boolean;
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
  onSave,
  onTest,
  canTest
}: EditorHeaderProps) {
  return (
    <header className="flex shrink-0 items-center gap-3 border-b border-border bg-panel px-4 py-2">
      <TextInput
        type="text"
        value={def.name}
        onChange={(event) => onNameChange(event.target.value)}
        aria-label="Nom du workflow"
        className="w-48 font-semibold"
      />
      <TextInput
        type="text"
        value={def.description ?? ''}
        onChange={(event) => onDescriptionChange(event.target.value)}
        placeholder="Description"
        aria-label="Description"
        className="grow"
      />
      <Badge tone="neutral">{ORIGIN_LABELS[origin]}</Badge>
      <Select
        value={scope}
        onChange={(event) => onScopeChange(event.target.value as 'project' | 'global')}
        aria-label="Portée d’enregistrement"
      >
        <option value="project">Projet</option>
        <option value="global">Global</option>
      </Select>
      {dirty && <span className="text-xs text-accent" title="Modifications non enregistrées">●</span>}
      {issueCount > 0 && <span className="text-xs text-err">{issueCount} problème(s)</span>}
      {onTest && (
        <Button
          variant="secondary"
          size="sm"
          type="button"
          onClick={onTest}
          disabled={!canTest}
          title={canTest ? "Ouvre une nouvelle session avec ce workflow, en pas à pas." : "Enregistre le workflow avant de le tester."}
        >
          Tester pas à pas
        </Button>
      )}
      <Button variant="primary" size="sm" type="button" onClick={onSave} disabled={saving}>
        {saving ? 'Enregistrement…' : 'Enregistrer'}
      </Button>
    </header>
  );
}
