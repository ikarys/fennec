// Pure helpers for the split view's per-pane tab lists.
export type PaneSide = 'left' | 'right';

export interface PaneTabs {
  left: string[];
  right: string[];
}

/** The pane lists after the split's panes changed: seeded on entry, each pane's shown tab appended if missing. Returns `previous` itself when nothing changed. */
export function syncPaneTabs(previous: PaneTabs | null, left: string | null, right: string | null, seed: string[]): PaneTabs {
  const base = previous ?? { left: seed.filter((id) => !id.startsWith('page:')), right: [] };
  const withTab = (list: string[], id: string | null): string[] => (id && !list.includes(id) ? [...list, id] : list);
  const nextLeft = withTab(base.left, left);
  const nextRight = withTab(base.right, right);

  if (previous && nextLeft === previous.left && nextRight === previous.right) return previous;
  return { left: nextLeft, right: nextRight };
}

/** Closes a tab in one pane: the new lists and the tab that pane falls back to (its last remaining one, or null). */
export function closePaneTab(tabs: PaneTabs, side: PaneSide, id: string): { tabs: PaneTabs; fallback: string | null } {
  const remaining = tabs[side].filter((tabId) => tabId !== id);
  const next = side === 'left' ? { ...tabs, left: remaining } : { ...tabs, right: remaining };

  return { tabs: next, fallback: remaining[remaining.length - 1] ?? null };
}

/** A pane's pseudo tab (`new:<project>`) became a real session. */
export function replacePaneTab(tabs: PaneTabs, from: string, to: string): PaneTabs {
  return {
    left: tabs.left.map((id) => (id === from ? to : id)),
    right: tabs.right.map((id) => (id === from ? to : id)),
  };
}
