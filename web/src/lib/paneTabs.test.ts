// The split view's per-pane tab lists: seeding, closing on one side, pseudo tabs.
import { describe, expect, it } from 'vitest';
import { closePaneTab, replacePaneTab, syncPaneTabs } from './paneTabs';

describe('paneTabs', () => {
  it('seeds the left pane from global session tabs and appends the right pane session', () => {
    const next = syncPaneTabs(null, 'left-session', 'right-session', ['left-session', 'page:settings']);

    expect(next.left).toEqual(['left-session']);
    expect(next.right).toEqual(['right-session']);
  });

  it('closes a tab in the left pane without touching the right pane', () => {
    const tabs = { left: ['a', 'b', 'c'], right: ['c'] };
    const result = closePaneTab(tabs, 'left', 'a');

    expect(result.tabs.left).toEqual(['b', 'c']);
    expect(result.tabs.right).toEqual(['c']);
    expect(result.fallback).toBe('c');
  });

  it('keeps a tab in the other pane when closing it on one side', () => {
    const tabs = { left: ['shared'], right: ['shared'] };
    const result = closePaneTab(tabs, 'left', 'shared');

    expect(result.tabs.left).toEqual([]);
    expect(result.tabs.right).toEqual(['shared']);
    expect(result.fallback).toBeNull();
  });

  it('returns the previous object when nothing changed', () => {
    const previous = { left: ['a'], right: ['b'] };
    const next = syncPaneTabs(previous, 'a', 'b', ['a']);

    expect(next).toBe(previous);
  });

  it('swaps a pseudo id in both lists', () => {
    const tabs = { left: ['new:p1', 'a'], right: ['new:p1'] };
    const next = replacePaneTab(tabs, 'new:p1', 'real');

    expect(next.left).toEqual(['real', 'a']);
    expect(next.right).toEqual(['real']);
  });
});
