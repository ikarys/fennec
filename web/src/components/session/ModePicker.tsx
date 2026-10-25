import type { Agent } from '../../lib/api';

interface ModePickerProps {
  agents: Agent[];
  mode: string | null;
  onChange: (id: string) => void;
  disabled?: boolean;
}

/**
 * The session's top-level agent. Switching applies from the next turn, so the
 * control is not disabled just because a turn is running.
 */
export function ModePicker({ agents, mode, onChange, disabled }: ModePickerProps) {
  // A single agent is not a choice; the server's default is enough.
  if (agents.length < 2) {
    return null;
  }

  return (
    <div role="group" aria-label="Mode" className="flex items-center gap-0.5 rounded-lg border border-border bg-panel p-0.5">
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
            className={`flex min-h-7 items-center gap-1.5 rounded-md px-2 text-xs transition-colors disabled:opacity-60 ${
              selected ? 'bg-raised text-text' : 'text-muted hover:text-text'
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
