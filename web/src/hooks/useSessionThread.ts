import { useCallback, useEffect, useRef, useState } from 'react';
import { getEventPage, getRunEventsBefore, type EventPageEntry } from '../lib/api';
import { createThreadFold, type FennecEvent, type ThreadFold, type ThreadView } from '../lib/fold';

const PAGE_SIZE = 200;

const EMPTY: ThreadView = createThreadFold().view();

export interface SessionThread {
  view: ThreadView;
  /** The log has events older than the loaded pages. */
  hasMore: boolean;
  loadingOlder: boolean;
  /** Prepend the previous batch; resolves true when the view changed. */
  loadOlder: () => Promise<boolean>;
}

/**
 * The live thread of a session: the last page of the log (at least 200 events,
 * from the start of a turn) is loaded as JSON, the rest follows on the SSE
 * stream, and older pages are fetched on demand. The raw events of the loaded
 * pages are kept so an older page can be prepended by re-folding; the unloaded
 * history costs no memory. A run that started before the loaded page keeps its
 * strip: its `workflow.*` events are fetched once and fed to the run fold only.
 */
export function useSessionThread(sessionId: string | null): SessionThread {
  const [view, setView] = useState<ThreadView>(EMPTY);
  const [hasMore, setHasMore] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const eventsRef = useRef<FennecEvent[]>([]);
  const foldRef = useRef<ThreadFold>(createThreadFold());
  const firstSeqRef = useRef(0);
  const lastSeqRef = useRef(0);
  const runPrefixRef = useRef<EventPageEntry[]>([]);

  const foldAll = useCallback(() => {
    const fresh = createThreadFold();
    const firstSeq = firstSeqRef.current;
    for (const entry of runPrefixRef.current) {
      if (entry.seq < firstSeq) {
        fresh.applyRunOnly(entry.event);
      }
    }
    for (const event of eventsRef.current) {
      fresh.apply(event);
    }
    foldRef.current = fresh;
    setView(fresh.view());
  }, []);

  useEffect(() => {
    eventsRef.current = [];
    foldRef.current = createThreadFold();
    firstSeqRef.current = 0;
    lastSeqRef.current = 0;
    runPrefixRef.current = [];
    setView(EMPTY);
    setHasMore(false);
    if (!sessionId) return;

    let cancelled = false;
    let source: EventSource | null = null;
    // One render per animation frame: a replay or a fast stream delivers
    // many events, and each render re-lays out the thread.
    let frame = 0;
    const apply = (event: FennecEvent) => {
      foldRef.current.apply(event);
      if (!frame) {
        frame = requestAnimationFrame(() => {
          frame = 0;
          setView(foldRef.current.view());
        });
      }
    };
    const openStream = (after: number) => {
      if (source || cancelled) return;
      source = new EventSource(`/api/sessions/${encodeURIComponent(sessionId)}/events?after=${after}`);
      source.onmessage = (message) => {
        // On reconnect the server resumes after Last-Event-ID; this guards
        // against a replayed event all the same.
        const seq = Number(message.lastEventId);
        if (!Number.isFinite(seq) || seq <= lastSeqRef.current) return;
        lastSeqRef.current = seq;
        try {
          const event = JSON.parse(message.data) as FennecEvent;
          eventsRef.current.push(event);
          apply(event);
        } catch {
          // a malformed payload is skipped, not fatal
        }
      };
    };

    getEventPage(sessionId, 0, PAGE_SIZE)
      .then((page) => {
        if (cancelled) return;
        for (const entry of page) {
          eventsRef.current.push(entry.event);
          lastSeqRef.current = entry.seq;
        }
        if (page.length > 0) firstSeqRef.current = page[0].seq;
        foldAll();
        setHasMore(page.length > 0 && page[0].seq > 1);
        openStream(lastSeqRef.current);
        if (page.length > 0 && page[0].seq > 1) {
          getRunEventsBefore(sessionId, page[0].seq)
            .then((prefix) => {
              if (cancelled) return;
              runPrefixRef.current = prefix;
              foldAll();
            })
            .catch(() => {
              // The run strip is a convenience; the thread keeps working.
            });
        }
      })
      .catch(() => {
        // The page fetch failed: follow the stream from the start instead.
        openStream(0);
      });

    return () => {
      cancelled = true;
      source?.close();
      cancelAnimationFrame(frame);
    };
  }, [sessionId, foldAll]);

  // Returns true when the view was updated, so the caller knows whether a
  // scroll restore is armed for the upcoming view change.
  const loadOlder = useCallback(async (): Promise<boolean> => {
    if (!sessionId || loadingOlder) return false;
    setLoadingOlder(true);
    try {
      const page = await getEventPage(sessionId, firstSeqRef.current, PAGE_SIZE);
      if (page.length === 0) {
        setHasMore(false);
        return false;
      }
      eventsRef.current = [...page.map((entry) => entry.event), ...eventsRef.current];
      firstSeqRef.current = page[0].seq;
      foldAll();
      setHasMore(page.length > 0 && page[0].seq > 1);
      return true;
    } catch {
      // The loaded page stays; the button can be tried again.
      return false;
    } finally {
      setLoadingOlder(false);
    }
  }, [sessionId, loadingOlder, foldAll]);

  return { view, hasMore, loadingOlder, loadOlder };
}
