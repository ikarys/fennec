import type { SlashCommand } from '../../lib/api';

export function matchCommands(text: string, commands: SlashCommand[]): SlashCommand[] {
  const match = text.match(/^\/(\S*)$/);
  if (!match) return [];
  const query = (match[1] ?? '').toLowerCase();
  const startsWith = commands.filter((command) => command.name.toLowerCase().startsWith(query));
  const contains = commands.filter(
    (command) => !command.name.toLowerCase().startsWith(query) && command.name.toLowerCase().includes(query)
  );
  return [...startsWith, ...contains];
}

interface SlashMenuProps {
  commands: SlashCommand[];
  selected: number;
  onSelect: (index: number) => void;
}

export function SlashMenu({ commands, selected, onSelect }: SlashMenuProps) {
  if (commands.length === 0) {
    return null;
  }

  return (
    <div role="listbox" aria-label="Commandes" className="max-h-56 overflow-y-auto border-b border-border py-1">
      {commands.map((command, index) => (
        <div
          key={command.name}
          role="option"
          aria-selected={index === selected}
          onClick={() => onSelect(index)}
          className={`flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-left text-sm ${
            index === selected ? 'bg-raised text-accent' : 'text-text'
          }`}
        >
          <span className="shrink-0 font-mono text-xs">{`/${command.name}`}</span>
          <span className="truncate text-xs text-muted">{command.description}</span>
        </div>
      ))}
    </div>
  );
}
