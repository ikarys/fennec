// Scrolls the session thread to a workflow node's divider, loading older pages until it is there.
import { useCallback, useEffect, useLayoutEffect, useRef } from 'react';
import { nodeAnchorId } from '../components/workflow/session/NodeDivider';

interface UseJumpToNodeArgs {
  view: unknown;
  hasMore: boolean;
  loadOlder: () => Promise<boolean>;
  setFollow: (on: boolean) => void;
}

/**
 * Returns `jump(runId, nodeId, pass)`: scrolls to the node run's divider, turning auto-follow off.
 * A divider not loaded yet is looked for page by page, back to the log's start.
 */
export function useJumpToNode({ view, hasMore, loadOlder, setFollow }: UseJumpToNodeArgs) {
  const target = useRef<string | null>(null);
  const latest = useRef({ hasMore, loadOlder, setFollow });
  useLayoutEffect(() => {
    latest.current = { hasMore, loadOlder, setFollow };
  });

  const tryJump = useCallback((id: string) => {
    const divider = document.getElementById(id);
    if (divider) {
      target.current = null;
      latest.current.setFollow(false);
      // Instant: a smooth scroll starts at the bottom, where its first scroll event turns follow back on.
      divider.scrollIntoView({ block: 'start' });
      return;
    }
    if (!latest.current.hasMore) {
      target.current = null;
      return;
    }
    void latest.current.loadOlder().then((loaded) => {
      if (!loaded && target.current === id) target.current = null;
    });
  }, []);

  // Each prepended page re-renders the thread: look again.
  useEffect(() => {
    if (target.current) tryJump(target.current);
  }, [view, tryJump]);

  return useCallback(
    (runId: string, nodeId: string, pass: number) => {
      target.current = nodeAnchorId(runId, nodeId, pass);
      tryJump(target.current);
    },
    [tryJump],
  );
}
