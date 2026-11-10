// The split view's per-pane tab lists: each pane keeps its own open tabs while the split lasts.
import { useCallback, useEffect, useRef, useState } from 'react';
import { closePaneTab, replacePaneTab, syncPaneTabs, type PaneSide, type PaneTabs } from '../lib/paneTabs';

export function usePaneTabs(split: { left: string | null; right: string | null } | null, seed: string[]) {
  const [paneTabs, setPaneTabs] = useState<PaneTabs | null>(null);
  const seedRef = useRef(seed);
  seedRef.current = seed;

  const active = split !== null;
  const left = split?.left ?? null;
  const right = split?.right ?? null;

  useEffect(() => {
    setPaneTabs((previous) => (active ? syncPaneTabs(previous, left, right, seedRef.current) : null));
  }, [active, left, right]);

  const closeTab = useCallback((side: PaneSide, id: string): string | null => {
    if (!paneTabs) return null;
    const result = closePaneTab(paneTabs, side, id);
    setPaneTabs(result.tabs);
    return result.fallback;
  }, [paneTabs]);

  const replaceTab = useCallback((from: string, to: string) => {
    setPaneTabs((previous) => (previous ? replacePaneTab(previous, from, to) : previous));
  }, []);

  return { paneTabs, closeTab, replaceTab };
}
