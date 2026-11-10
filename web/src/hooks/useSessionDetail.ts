// The session's detail (worktree, idea, proposals), fetched on demand: when the session changes, after a turn ends, or when a caller asks for a reload.
import { useEffect, useRef, useState } from 'react';
import { getSession } from '../lib/api';
import type { SessionDetail } from '../lib/api';

export function useSessionDetail(
  sessionId: string | null,
  running: boolean
): { detail: SessionDetail | null; detailVersion: number; reloadDetail: () => void } {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [detailVersion, setDetailVersion] = useState(0);
  const previousRunning = useRef(running);

  useEffect(() => {
    if (!sessionId) {
      setDetail(null);
      return;
    }
    let active = true;
    getSession(sessionId)
      .then((data) => {
        if (active) setDetail(data);
      })
      .catch(() => {
        // The event stream is the source of truth for the thread.
      });
    return () => {
      active = false;
    };
  }, [sessionId, detailVersion]);

  useEffect(() => {
    // A workflow's first turn creates the worktree: re-read the detail when
    // a turn ends, not only on a reload.
    if (previousRunning.current && !running) {
      setDetailVersion((version) => version + 1);
    }
    previousRunning.current = running;
  }, [running]);

  return { detail, detailVersion, reloadDetail: () => setDetailVersion((version) => version + 1) };
}
