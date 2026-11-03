import { Segmented } from '../ui/Segmented';
import { ChoiceDot, ChoiceMenu, type ChoiceGroup } from '../ui/ChoiceMenu';
import type { Agent, AgentPhase } from '../../lib/api';
import { t, type Lang } from '../../i18n/messages';
import { useLang, useT } from '../../i18n';

interface ModePickerProps {
  agents: Agent[];
  mode: string | null;
  onChange: (id: string) => void;
  disabled?: boolean;
}

const PHASE_KEYS: Record<AgentPhase, 'phaseStart' | 'phaseFrame' | 'phaseBuild' | 'phaseUnderstand' | 'phaseCheck'> = {
  start: 'phaseStart',
  frame: 'phaseFrame',
  build: 'phaseBuild',
  understand: 'phaseUnderstand',
  check: 'phaseCheck',
};

// The server already ordered agents by phase; group only consecutive runs so the
// menu mirrors that order without re-sorting in the UI.
function phaseGroups(lang: Lang, agents: Agent[]): ChoiceGroup[] {
  const groups: ChoiceGroup[] = [];
  for (const agent of agents) {
    const label = agent.phase ? t(lang, PHASE_KEYS[agent.phase]) : t(lang, 'phaseOther');
    const choice = { value: agent.id, label: agent.name, description: agent.description, color: agent.color };
    const last = groups[groups.length - 1];
    if (last && last.label === label) {
      last.choices.push(choice);
    } else {
      groups.push({ label, choices: [choice] });
    }
  }
  return groups;
}

/**
 * The session's top-level agent. Segmented up to three agents; beyond that a
 * menu, grouped by the project phase the server ordered them by, keeps the
 * composer narrow. Switching applies from the next turn.
 */
export function ModePicker({ agents, mode, onChange, disabled }: ModePickerProps) {
  const t = useT();
  const lang = useLang();
  // A single agent is not a choice; the server's default is enough.
  if (agents.length < 2) {
    return null;
  }

  // A segmented control stops fitting the composer.
  if (agents.length > 3) {
    const selected = agents.find((agent) => agent.id === mode);
    return (
      <ChoiceMenu
        label={t('sessionAgent')}
        title={selected ? t('sessionAgentOf', { name: `${selected.name}${selected.description ? ` — ${selected.description}` : ''}` }) : t('sessionAgent')}
        groups={phaseGroups(lang, agents)}
        value={mode}
        onChange={onChange}
        disabled={disabled}
        trigger={
          <>
            <ChoiceDot color={selected?.color ?? null} />
            {selected?.name ?? t('mode')}
          </>
        }
      />
    );
  }

  return (
    <Segmented
      label={t('sessionAgent')}
      value={mode}
      onChange={onChange}
      disabled={disabled}
      segments={agents.map((agent) => ({
        value: agent.id,
        title: t('sessionAgentOf', { name: `${agent.name}${agent.description ? ` — ${agent.description}` : ''}` }),
        label: (
          <>
            <ChoiceDot color={agent.color} />
            {agent.name}
          </>
        )
      }))}
    />
  );
}
