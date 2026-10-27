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
    <div role="group" aria-label="Mode" className="flex items-center gap-0.5 rounded-lg bg-raised/60 p-0.5">
      {agents.map((agent) => {
        const selected = agent.id === mode;
        return (
          <button
            key={agent.id}
            type="button"
            title={agent.description}
            aria-pressed={selected}
            disabled={disabled}
            onClick={() => onChange(agent.id)}
            className={`flex h-6 items-center gap-1.5 rounded-md px-2 text-xs transition-colors disabled:opacity-60 ${
              selected ? 'bg-panel text-text shadow-sm' : 'text-muted hover:text-text'
            }`}
          >
            <span
              className="h-1.5 w-1.5 rounded-full bg-muted"
              style={agent.color ? { backgroundColor: agent.color } : undefined}
            />
            {agent.name}
          </button>
        );
      })}
    </div>
  );
}
