// The editor page's open-file tabs: opening a file, closing one with its fallback.
import { describe, expect, it } from 'vitest';
import { closeFileTab, openFileTab } from './fileTabs';

describe('fileTabs', () => {
  it('opens a file into an empty tab list', () => {
    expect(openFileTab([], 'src/main.rs')).toEqual(['src/main.rs']);
  });

  it('appends a new file at the end, keeping the open order', () => {
    const open = openFileTab(['a.ts', 'b.ts'], 'c.ts');

    expect(open).toEqual(['a.ts', 'b.ts', 'c.ts']);
  });

  it('does not duplicate a file that is already open', () => {
    const open = openFileTab(['a.ts', 'b.ts'], 'a.ts');

    expect(open).toEqual(['a.ts', 'b.ts']);
  });

  it('closes a tab and falls back to the last remaining one', () => {
    const result = closeFileTab(['a.ts', 'b.ts', 'c.ts'], 'a.ts');

    expect(result.open).toEqual(['b.ts', 'c.ts']);
    expect(result.fallback).toBe('c.ts');
  });

  it('closes the active last tab and falls back to the previous one', () => {
    const result = closeFileTab(['a.ts', 'b.ts'], 'b.ts');

    expect(result.open).toEqual(['a.ts']);
    expect(result.fallback).toBe('a.ts');
  });

  it('returns an empty list and a null fallback when closing the last tab', () => {
    const result = closeFileTab(['a.ts'], 'a.ts');

    expect(result.open).toEqual([]);
    expect(result.fallback).toBeNull();
  });

  it('leaves the list untouched when closing a file that is not open', () => {
    const result = closeFileTab(['a.ts', 'b.ts'], 'c.ts');

    expect(result.open).toEqual(['a.ts', 'b.ts']);
    expect(result.fallback).toBe('b.ts');
  });
});
