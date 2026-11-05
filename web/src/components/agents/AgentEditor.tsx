/**
 * The agent form: the fields of a `*.agent.md`, never the file itself. The
 * server renders the frontmatter (`domain/agent_file.rs`), so the browser
 * never writes YAML.
 */
import { FieldRow, Select, TextArea, TextInput } from '../ui/Field';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';
import type { AgentDraft } from '../../lib/api/agents';
import type { ReasoningEffort } from '../../lib/api/settings';
import type { AgentPhase } from '../../lib/api/projects';
import type { MsgKey } from '../../i18n/messages';

const PHASES: { id: AgentPhase; label: MsgKey }[] = [
  { id: 'start', label: 'phaseStart' },
  { id: 'frame', label: 'phaseFrame' },
  { id: 'build', label: 'phaseBuild' },
  { id: 'understand', label: 'phaseUnderstand' },
  { id: 'check', label: 'phaseCheck' },
];

const EFFORTS: ReasoningEffort[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

/** A new agent reads and runs commands, and plans nothing it cannot explain. */
export const EMPTY_DRAFT: AgentDraft = {
  id: '',
  name: '',
  description: '',
  phase: 'build',
  workflow: null,
  color: null,
  subagent: false,
  readOnly: true,
  reasoningEffort: null,
  allowedTools: ['read_file', 'run_command', 'load_skill'],
  prompt: '',
};

export function AgentEditor({
  draft,
  tools,
  busy,
  error,
  field,
  newAgent,
  onChange,
  onSave,
  onCancel,
}: {
  draft: AgentDraft;
  tools: string[];
  busy: boolean;
  error: string | null;
  /** The input the server pointed at, highlighted. */
  field: string | null;
  newAgent: boolean;
  onChange: (draft: AgentDraft) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  const set = <K extends keyof AgentDraft>(key: K, value: AgentDraft[K]) =>
    onChange({ ...draft, [key]: value });

  const toggleTool = (tool: string) =>
    set(
      'allowedTools',
      draft.allowedTools.includes(tool)
        ? draft.allowedTools.filter((name) => name !== tool)
        : [...draft.allowedTools, tool],
    );

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <FieldRow label={t('agentIdLabel')} htmlFor="agent-id" hint={t('agentIdHint')}>
          <TextInput
            id="agent-id"
            className={`w-full font-mono${field === 'id' ? ' border-err' : ''}`}
            value={draft.id}
            placeholder={t('agentIdPlaceholder')}
            disabled={!newAgent}
            onChange={(e) => set('id', e.target.value)}
          />
        </FieldRow>
        <FieldRow label={t('nameLabel')} htmlFor="agent-name">
          <TextInput
            id="agent-name"
            className="w-full"
            value={draft.name}
            placeholder={t('agentNamePlaceholder')}
            onChange={(e) => set('name', e.target.value)}
          />
        </FieldRow>
        <FieldRow label={t('agentPhaseLabel')} htmlFor="agent-phase" hint={t('agentPhaseHint')}>
          <Select
            id="agent-phase"
            className="w-full"
            value={draft.phase ?? ''}
            onChange={(e) => set('phase', (e.target.value || null) as AgentPhase | null)}
          >
            <option value="">{t('agentPhaseNone')}</option>
            {PHASES.map((phase) => (
              <option key={phase.id} value={phase.id}>
                {t(phase.label)}
              </option>
            ))}
          </Select>
        </FieldRow>
        <FieldRow label={t('agentColorLabel')} htmlFor="agent-color" hint={t('agentColorHint')}>
          <TextInput
            id="agent-color"
            className="w-full font-mono"
            value={draft.color ?? ''}
            placeholder="#a855f7"
            onChange={(e) => set('color', e.target.value || null)}
          />
        </FieldRow>
      </div>

      <FieldRow label={t('agentDescriptionLabel')} htmlFor="agent-description" hint={t('agentDescriptionHint')}>
        <TextInput
          id="agent-description"
          className="w-full"
          value={draft.description}
          onChange={(e) => set('description', e.target.value)}
        />
      </FieldRow>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <FieldRow label={t('agentCommandsLabel')} htmlFor="agent-commands">
          <Select
            id="agent-commands"
            className="w-full"
            value={draft.readOnly ? 'readOnly' : 'all'}
            onChange={(e) => set('readOnly', e.target.value === 'readOnly')}
          >
            <option value="readOnly">{t('agentCommandsReadOnly')}</option>
            <option value="all">{t('agentCommandsWrite')}</option>
          </Select>
        </FieldRow>
        <FieldRow label={t('effortLabel')} htmlFor="agent-effort">
          <Select
            id="agent-effort"
            className="w-full"
            value={draft.reasoningEffort ?? ''}
            onChange={(e) => set('reasoningEffort', (e.target.value || null) as ReasoningEffort | null)}
          >
            <option value="">{t('agentEffortModel')}</option>
            {EFFORTS.map((effort) => (
              <option key={effort} value={effort}>
                {effort}
              </option>
            ))}
          </Select>
        </FieldRow>
        <FieldRow label={t('launch')} htmlFor="agent-subagent" hint={t('agentSubagentHint')}>
          <Select
            id="agent-subagent"
            className="w-full"
            value={draft.subagent ? 'subagent' : 'mode'}
            onChange={(e) => set('subagent', e.target.value === 'subagent')}
          >
            <option value="mode">{t('agentLaunchMode')}</option>
            <option value="subagent">{t('agentLaunchSubagent')}</option>
          </Select>
        </FieldRow>
        <FieldRow label={t('agentWorkflowLabel')} htmlFor="agent-workflow" hint={t('agentWorkflowHint')}>
          <TextInput
            id="agent-workflow"
            className="w-full font-mono"
            value={draft.workflow ?? ''}
            placeholder="plan-build-verify"
            onChange={(e) => set('workflow', e.target.value || null)}
          />
        </FieldRow>
      </div>

      <FieldRow label={t('agentToolsLabel')}>
        <div className={`flex flex-wrap gap-x-4 gap-y-1.5${field === 'allowedTools' ? ' rounded-md border border-err p-2' : ''}`}>
          {tools.map((tool) => (
            <label key={tool} className="flex items-center gap-1.5 text-sm">
              <input
                type="checkbox"
                checked={draft.allowedTools.includes(tool)}
                onChange={() => toggleTool(tool)}
              />
              <span className="font-mono text-xs">{tool}</span>
            </label>
          ))}
        </div>
      </FieldRow>

      <FieldRow label={t('agentMethodLabel')} htmlFor="agent-prompt" hint={t('agentMethodHint')}>
        <TextArea
          id="agent-prompt"
          className={`w-full text-xs${field === 'prompt' ? ' border-err' : ''}`}
          rows={16}
          value={draft.prompt}
          onChange={(e) => set('prompt', e.target.value)}
        />
      </FieldRow>

      {error ? <p className="text-xs text-err">{error}</p> : null}
      <div className="flex gap-2">
        <Button
          variant="primary"
          onClick={onSave}
          disabled={busy || !draft.id.trim() || !draft.name.trim() || !draft.prompt.trim()}
        >
          {t('agentSave')}
        </Button>
        <Button variant="ghost" onClick={onCancel} disabled={busy}>
          {t('cancel')}
        </Button>
      </div>
    </div>
  );
}
