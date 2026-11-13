import type { Origin, WorkflowDef } from '../../lib/workflow';
import type { MsgKey } from '../../i18n/messages';
import { useT } from '../../i18n';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { PlayIcon, StepIcon } from '../ui/Icons';
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
  onLaunch: ((stepByStep: boolean) => void) | null;
  canTest: boolean;
}

const ORIGIN_LABELS: Record<Origin, MsgKey> = {
  project: 'workflowOriginProject',
  global: 'workflowOriginGlobal',
  default: 'workflowOriginDefault'
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
  onLaunch,
  canTest
}: EditorHeaderProps) {
  const t = useT();
  return (
    <header className="flex shrink-0 items-center gap-3 border-b border-border bg-panel px-4 py-2">
      <TextInput
        type="text"
        value={def.name}
        onChange={(event) => onNameChange(event.target.value)}
        aria-label={t('workflowEditorNameAria')}
        className="w-48 font-semibold"
      />
      <TextInput
        type="text"
        value={def.description ?? ''}
        onChange={(event) => onDescriptionChange(event.target.value)}
        placeholder={t('agentDescriptionLabel')}
        aria-label={t('agentDescriptionLabel')}
        className="grow"
      />
      <Badge tone="neutral">{t(ORIGIN_LABELS[origin])}</Badge>
      <Select
        value={scope}
        onChange={(event) => onScopeChange(event.target.value as 'project' | 'global')}
        aria-label={t('workflowEditorScopeAria')}
      >
        <option value="project">{t('workflowOriginProject')}</option>
        <option value="global">{t('workflowOriginGlobal')}</option>
      </Select>
      {dirty && <span className="text-xs text-accent" title={t('sectionUnsavedChanges')}>●</span>}
      {issueCount > 0 && <span className="text-xs text-err">{t('workflowEditorIssueCount', { count: issueCount })}</span>}
      {onLaunch && (
        <>
          <Button
            variant="secondary"
            size="sm"
            type="button"
            iconOnly
            onClick={() => onLaunch(false)}
            disabled={!canTest}
            aria-label={t('workflowEditorRunAria')}
            title={canTest ? t('workflowEditorRunTitle') : t('workflowEditorTestSaveTitle')}
          >
            <PlayIcon className="h-4 w-4" />
          </Button>
          <Button
            variant="secondary"
            size="sm"
            type="button"
            iconOnly
            onClick={() => onLaunch(true)}
            disabled={!canTest}
            aria-label={t('workflowEditorTestAria')}
            title={canTest ? t('workflowEditorTestTitle') : t('workflowEditorTestSaveTitle')}
          >
            <StepIcon className="h-4 w-4" />
          </Button>
        </>
      )}
      <Button variant="primary" size="sm" type="button" onClick={onSave} disabled={saving}>
        {saving ? t('sectionSaving') : t('agentSave')}
      </Button>
    </header>
  );
}
