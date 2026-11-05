import { describe, it, expect } from 'vitest';
import { matchCommands } from './SlashMenu';
import type { SlashCommand } from '../../lib/api';

describe('matchCommands', () => {
  it('returns commands before skills while keeping stable order within each kind', () => {
    const commands = [
      { name: 'alpha', description: 'a', kind: 'skill' },
      { name: 'beta', description: 'b', kind: 'command' },
      { name: 'gamma', description: 'c', kind: 'command' },
      { name: 'delta', description: 'd', kind: 'skill' }
    ] as SlashCommand[];

    const result = matchCommands('/', commands);

    expect(result.map((command) => command.kind)).toEqual(['command', 'command', 'skill', 'skill']);
    expect(result.map((command) => command.name)).toEqual(['beta', 'gamma', 'alpha', 'delta']);
  });

  it('keeps startsWith matches before contains matches within each kind', () => {
    const commands = [
      { name: 'shell', description: 's', kind: 'command' },
      { name: 'help', description: 'h', kind: 'command' },
      { name: 'sheel', description: 's', kind: 'skill' },
      { name: 'heal', description: 'h', kind: 'skill' }
    ] as SlashCommand[];

    const result = matchCommands('/he', commands);

    expect(result.map((command) => command.kind)).toEqual(['command', 'command', 'skill', 'skill']);
    expect(result.map((command) => command.name)).toEqual(['help', 'shell', 'heal', 'sheel']);
  });

  it('returns [] for non-slash text', () => {
    const commands = [
      { name: 'help', description: 'h', kind: 'command' },
      { name: 'skill-help', description: 's', kind: 'skill' }
    ] as SlashCommand[];

    expect(matchCommands('help', commands)).toEqual([]);
    expect(matchCommands('', commands)).toEqual([]);
  });
});
