import { Segmented } from '../ui/Segmented';
import { ChoiceDot, ChoiceMenu } from '../ui/ChoiceMenu';
import type { Agent } from '../../lib/api';
import { useT } from '../../i18n';

interface ModePickerProps {
  agents: Agent[];
  mode: string | null;
  onChange: (id: string) => void;
  disabled?: boolean;
}

/**
 * The session's top-level agent. Segmented up to three agents; beyond that a
 * flat menu, in the server's phase order, keeps the composer narrow.
 * Switching applies from the next turn.
 */
export function ModePicker({ agents, mode, onChange, disabled }: ModePickerProps) {
  const t = useT();
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
        groups={[{ choices: agents.map((agent) => ({ value: agent.id, label: agent.name, description: agent.description, color: agent.color })) }]}
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
