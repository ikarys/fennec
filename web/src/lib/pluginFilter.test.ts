// Vitest tests for plugin filtering and status counts.
import { describe, expect, it } from 'vitest';
import { filterPlugins, pluginStatusCounts } from './pluginFilter';

const plugins = [
  { name: 'Search', command: 'search', args: ['--web'], enabled: true },
  { name: 'Deploy', command: 'deploy', args: ['production'], enabled: false },
];

describe('filterPlugins', () => {
  it('matches the query on name', () => {
    expect(filterPlugins(plugins, 'sear', 'all')).toEqual([plugins[0]]);
  });

  it('matches the query on args', () => {
    expect(filterPlugins(plugins, 'production', 'all')).toEqual([plugins[1]]);
  });

  it('is case-insensitive', () => {
    expect(filterPlugins(plugins, 'SEARCH', 'all')).toEqual([plugins[0]]);
  });

  it('returns all plugins for an empty query', () => {
    expect(filterPlugins(plugins, '   ', 'all')).toEqual(plugins);
  });

  it('filters enabled plugins', () => {
    expect(filterPlugins(plugins, '', 'enabled')).toEqual([plugins[0]]);
  });

  it('filters disabled plugins', () => {
    expect(filterPlugins(plugins, '', 'disabled')).toEqual([plugins[1]]);
  });
});

describe('pluginStatusCounts', () => {
  it('counts all, enabled, and disabled plugins', () => {
    expect(pluginStatusCounts(plugins)).toEqual({ all: 2, enabled: 1, disabled: 1 });
  });
});
