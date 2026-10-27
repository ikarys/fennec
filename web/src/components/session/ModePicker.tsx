import { Segmented } from '../ui/Segmented';
import type { Agent } from '../../lib/api';

interface ModePickerProps {
  agents: Agent[];
  mode: string | null;
  onChange: (id: string) => void;
  disabled?: boolean;
}

/**
 * The session's top-level agent. Segmented up to three agents; beyond that a
 * select keeps the composer narrow. Switching applies from the next turn.
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
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id} title={agent.description}>
              {agent.name}
            </option>
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
