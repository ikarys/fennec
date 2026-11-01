import { Segmented } from '../ui/Segmented';
import type { Agent, AgentPhase } from '../../lib/api';

interface ModePickerProps {
  agents: Agent[];
  mode: string | null;
  onChange: (id: string) => void;
  disabled?: boolean;
}

const PHASE_LABELS: Record<AgentPhase, string> = {
  start: 'Démarrer',
  frame: 'Cadrer',
  build: 'Construire',
  understand: 'Comprendre',
  check: 'Contrôler',
};

interface PhaseGroup {
  label: string;
  agents: Agent[];
}

// The server already ordered agents by phase; group only consecutive runs so the
// select mirrors that order without re-sorting in the UI.
function phaseGroups(agents: Agent[]): PhaseGroup[] {
  const groups: PhaseGroup[] = [];
  for (const agent of agents) {
    const label = agent.phase ? PHASE_LABELS[agent.phase] : 'Autres';
    const last = groups[groups.length - 1];
    if (last && last.label === label) {
      last.agents.push(agent);
    } else {
      groups.push({ label, agents: [agent] });
    }
  }
  return groups;
}

/**
 * The session's top-level agent. Segmented up to three agents; beyond that a
 * select, grouped by the project phase the server ordered them by, keeps the
 * composer narrow. Switching applies from the next turn.
 */
export function ModePicker({ agents, mode, onChange, disabled }: ModePickerProps) {
  // A single agent is not a choice; the server's default is enough.
  if (agents.length < 2) {
    return null;
  }

  // A segmented control stops fitting the composer.
  if (agents.length > 3) {
    const selectedAgent = agents.find((agent) => agent.id === mode);
    return (
      <div className="flex items-center gap-1.5">
        <span
          className="h-1.5 w-1.5 rounded-full bg-muted"
          style={selectedAgent?.color ? { backgroundColor: selectedAgent.color } : undefined}
        />
        <select
          aria-label="Mode"
          value={mode ?? ''}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          className="h-7 rounded-md bg-transparent px-1.5 text-xs text-muted outline-none transition-colors hover:bg-raised hover:text-text"
        >
          {phaseGroups(agents).map((group, index) => (
            <optgroup key={`${group.label}-${index}`} label={group.label}>
              {group.agents.map((agent) => (
                <option key={agent.id} value={agent.id} title={agent.description}>
                  {agent.name}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </div>
    );
  }

  return (
    <Segmented
      label="Mode"
      value={mode}
      onChange={onChange}
      disabled={disabled}
      segments={agents.map((agent) => ({
        value: agent.id,
        title: agent.description,
        label: (
          <>
            <span className="h-1.5 w-1.5 rounded-full bg-muted" style={agent.color ? { backgroundColor: agent.color } : undefined} />
            {agent.name}
          </>
        )
      }))}
    />
  );
}
