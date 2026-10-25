import { useEffect, useState } from 'react';
import { createThreadFold, type FennecEvent, type ThreadView } from '../lib/fold';

const EMPTY: ThreadView = createThreadFold().view();

/** The live thread of a session, folded from its event stream as it arrives. */
export function useSessionThread(sessionId: string | null): ThreadView {
  const [view, setView] = useState<ThreadView>(EMPTY);

  useEffect(() => {
    setView(EMPTY);
    if (!sessionId) return;

    const fold = createThreadFold();
    let lastSeq = 0;
    // One render per animation frame: a replay or a fast stream delivers
    // many events, and each render re-lays out the thread.
    let frame = 0;
    const flush = () => {
      frame = 0;
      setView(fold.view());
    };
    const source = new EventSource(`/api/sessions/${encodeURIComponent(sessionId)}/events`);
    source.onmessage = (message) => {
      // On reconnect the server resumes after Last-Event-ID; this guards
      // against a replayed event all the same.
      const seq = Number(message.lastEventId);
      if (Number.isFinite(seq) && seq <= lastSeq) return;
      lastSeq = seq;
      try {
        fold.apply(JSON.parse(message.data) as FennecEvent);
      } catch {
        return; // a malformed payload is skipped, not fatal
      }
      if (!frame) frame = requestAnimationFrame(flush);
    };

    return () => {
      source.close();
      cancelAnimationFrame(frame);
    };
  }, [sessionId]);

  return view;
}
