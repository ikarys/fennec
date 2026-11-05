import { Fragment } from 'react';
import type { SlashCommand } from '../../lib/api';
import { SectionLabel } from '../ui/SectionLabel';
import { useT } from '../../i18n';

export function matchCommands(text: string, commands: SlashCommand[]): SlashCommand[] {
  const match = text.match(/^\/(\S*)$/);
  if (!match) return [];

  const query = (match[1] ?? '').toLowerCase();
  const startsWith = commands.filter((command) => command.name.toLowerCase().startsWith(query));
  const contains = commands.filter(
    (command) => !command.name.toLowerCase().startsWith(query) && command.name.toLowerCase().includes(query)
  );

  // Filter first, then move commands before skills while preserving each kind's internal order.
  const filtered = [...startsWith, ...contains];
  return [
    ...filtered.filter((command) => command.kind === 'command'),
    ...filtered.filter((command) => command.kind === 'skill')
  ];
}

interface SlashMenuProps {
  commands: SlashCommand[];
  selected: number;
  onSelect: (index: number) => void;
}

export function SlashMenu({ commands, selected, onSelect }: SlashMenuProps) {
  const t = useT();

  if (commands.length === 0) {
    return null;
  }

  return (
    <div role="listbox" aria-label={t('slashGroupCommands')} className="max-h-56 overflow-y-auto border-b border-border py-1">
      {commands.map((command, index) => {
        const isFirstOfKind = index === 0 || commands[index - 1]?.kind !== command.kind;

        return (
          <Fragment key={command.name}>
            {isFirstOfKind && (
              <div role="presentation" className="px-3 pt-2 pb-1">
                <SectionLabel>
                  {command.kind === 'command' ? t('slashGroupCommands') : t('slashGroupSkills')}
                </SectionLabel>
              </div>
            )}
            <div
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
          </Fragment>
        );
      })}
    </div>
  );
}
