import { useEffect, useState } from 'react';
import type { FennecEvent } from '../lib/fold';

export function useSessionEvents(sessionId: string | null): { events: FennecEvent[]; running: boolean } {
  const [events, setEvents] = useState<FennecEvent[]>([]);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    setEvents([]);
    if (!sessionId) {
      setRunning(false);
      return;
    }

    const seen = new Set<number>();
    const localEvents: FennecEvent[] = [];
    // One render per animation frame: a replay or a fast stream delivers
    // thousands of events, and copying the list for each is quadratic.
    let frame = 0;
    const flush = () => {
      frame = 0;
      setEvents([...localEvents]);
    };
    const source = new EventSource(`/api/sessions/${encodeURIComponent(sessionId)}/events`);

    const addEvent = (event: FennecEvent) => {
      if (event.seq != null) {
        if (seen.has(event.seq)) return;
        seen.add(event.seq);
      }
      localEvents.push(event);
      if (event.type === 'running.changed') {
        setRunning(Boolean((event.data as { isRunning?: boolean }).isRunning));
      }
      if (!frame) frame = requestAnimationFrame(flush);
    };

    source.onmessage = (messageEvent) => {
      try {
        const data = JSON.parse(messageEvent.data);
        const seq = Number(messageEvent.lastEventId);
        addEvent({ ...data, seq: Number.isFinite(seq) ? seq : undefined });
      } catch {
        // Ignore malformed SSE payloads.
      }
    };

    return () => {
      source.close();
      cancelAnimationFrame(frame);
    };
  }, [sessionId]);

  return { events, running };
}
