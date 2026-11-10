import { describe, expect, it } from 'vitest';
import { SETTINGS_GROUPS, resolveSettingsPage } from './settingsPages';

describe('settingsPages', () => {
  it('resolves every listed page to itself', () => {
    for (const group of SETTINGS_GROUPS) {
      for (const page of group.pages) {
        expect(resolveSettingsPage(page)).toBe(page);
      }
    }
  });

  it('maps old addresses to their pages', () => {
    const legacy = [
      ['models', 'providers'],
      ['behaviour', 'instructions'],
      ['security', 'sandbox'],
      ['extensions', 'plugins'],
      ['library', 'skills']
    ] as const;

    for (const [input, expected] of legacy) {
      expect(resolveSettingsPage(input)).toBe(expected);
    }
  });

  it('falls back to providers for missing, empty, unknown and prototype-polluting ids', () => {
    expect(resolveSettingsPage(null)).toBe('providers');
    expect(resolveSettingsPage('')).toBe('providers');
    expect(resolveSettingsPage('nope')).toBe('providers');
    expect(resolveSettingsPage('constructor')).toBe('providers');
    expect(resolveSettingsPage('__proto__')).toBe('providers');
  });

  it('lists every page exactly once', () => {
    const pages = SETTINGS_GROUPS.flatMap((group) => group.pages);
    expect(new Set(pages).size).toBe(pages.length);
  });
});
