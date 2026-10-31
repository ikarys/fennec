import { useCallback, useEffect, useRef, useState } from 'react';
import { getContext, getVision, type ContextView, getEventPage, getRunEventsBefore, getSkills, getUsage, getUsageCalls, getUsageWorkflow, type WorkflowUsage, type EventPageEntry, type SessionSkills, type CallUsage, type SessionUsage, type SeenImage } from '../lib/api';
import { createThreadFold, type FennecEvent, type ThreadFold, type ThreadView } from '../lib/fold';

const PAGE_SIZE = 200;

const EMPTY: ThreadView = createThreadFold().view();

// A page is folded without `apply`: a load_skill call still in flight in it
// must be tracked too, so its result, arriving by SSE, refetches the skills.
function trackLoadSkillCalls(entries: EventPageEntry[], tracked: Set<string>): void {
  for (const entry of entries) {
    const data: FennecEvent['data'] = entry.event.data ?? {};
    if (entry.event.type === 'tool.call' && data.toolCall?.name === 'load_skill') {
      tracked.add(String(data.toolCall?.id ?? ''));
    }
  }
}

export interface SessionThread {
  view: ThreadView;
  /** The log has events older than the loaded pages. */
  hasMore: boolean;
  loadingOlder: boolean;
  /** Prepend the previous batch; resolves true when the view changed. */
  loadOlder: () => Promise<boolean>;
  /** The session's skills folded server-side; null while the first fetch is in flight. */
  skills: SessionSkills | null;
  /** The session's token totals from the server's usage ledger; null until the first fetch. */
  usage: SessionUsage | null;
  /** Each answer's ledger figures by message id. */
  calls: ReadonlyMap<string, CallUsage>;
  workflowUsage: WorkflowUsage;
  /** The header's context gauge; null until the first fetch. */
  context: ContextView | null;
  /** The images the model received, computed server-side; [] until the first fetch. */
  vision: SeenImage[];
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
  const [skills, setSkills] = useState<SessionSkills | null>(null);
  const [usage, setUsage] = useState<SessionUsage | null>(null);
  const [calls, setCalls] = useState<ReadonlyMap<string, CallUsage>>(new Map());
  const [workflowUsage, setWorkflowUsage] = useState<WorkflowUsage>({ nodes: [], runs: [] });
  const [context, setContext] = useState<ContextView | null>(null);
  const [vision, setVision] = useState<SeenImage[]>([]);
  const eventsRef = useRef<FennecEvent[]>([]);
  const foldRef = useRef<ThreadFold>(createThreadFold());
  const firstSeqRef = useRef(0);
  const lastSeqRef = useRef(0);
  const runPrefixRef = useRef<EventPageEntry[]>([]);
  const loadSkillCallsRef = useRef(new Set<string>());
  // A counter: only the latest fetch may answer, so out-of-order responses
  // cannot regress the panel.
  const skillsReqRef = useRef(0);
  const usageReqRef = useRef(0);
  const contextReqRef = useRef(0);
  const visionReqRef = useRef(0);

  const refreshSkills = useCallback(() => {
    if (!sessionId) return;
    const request = ++skillsReqRef.current;
    getSkills(sessionId)
      .then((data) => {
        if (request === skillsReqRef.current) setSkills(data);
      })
      .catch(() => {
        // A stale failure must not clobber a newer answer. With no data yet,
        // fall back to the empty state instead of a perpetual spinner; with
        // data, keep the last skills.
        if (request === skillsReqRef.current) {
          setSkills((prev) => prev ?? { loaded: [], active: [], alwaysOn: [], missedAlwaysOn: [] });
        }
      });
  }, [sessionId]);

  const refreshUsage = useCallback(() => {
    if (!sessionId) return;
    const request = ++usageReqRef.current;
    Promise.all([getUsage(sessionId), getUsageCalls(sessionId), getUsageWorkflow(sessionId)])
      .then(([totals, perCall, perWorkflow]) => {
        if (request !== usageReqRef.current) return;
        setUsage(totals);
        setCalls(new Map(perCall.map((call) => [call.messageId, call])));
        setWorkflowUsage(perWorkflow);
      })
      .catch(() => {
        // Keep the last totals: the next model call refetches.
      });
  }, [sessionId]);

  const refreshContext = useCallback(() => {
    if (!sessionId) return;
    const request = ++contextReqRef.current;
    getContext(sessionId)
      .then((view) => {
        if (request === contextReqRef.current) setContext(view);
      })
      .catch(() => {
        // Keep the last gauge: the next context.state refetches.
      });
  }, [sessionId]);

  const refreshVision = useCallback(() => {
    if (!sessionId) return;
    const request = ++visionReqRef.current;
    getVision(sessionId)
      .then((images) => {
        if (request === visionReqRef.current) setVision(images);
      })
      .catch(() => {
        // Keep the last images: the next image or cut refetches.
      });
  }, [sessionId]);

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
    loadSkillCallsRef.current = new Set();
    skillsReqRef.current += 1; // a fetch of the previous session may not answer
    usageReqRef.current += 1;
    contextReqRef.current += 1;
    visionReqRef.current += 1;
    setView(EMPTY);
    setHasMore(false);
    setSkills(null);
    setUsage(null);
    setCalls(new Map());
    setWorkflowUsage({ nodes: [], runs: [] });
    setContext(null);
    setVision([]);
    if (!sessionId) return;

    let cancelled = false;
    let source: EventSource | null = null;
    // One render per animation frame: a replay or a fast stream delivers
    // many events, and each render re-lays out the thread.
    let frame = 0;
    const apply = (event: FennecEvent) => {
      foldRef.current.apply(event);
      const data = event.data ?? {};
      // Refetch only when the fold can change: a skill load succeeds (a
      // tool.result for a tracked load_skill), a user message requests a skill,
      // or the model's view is cut (compaction, replay). An in-flight load_skill
      // call loads nothing until it succeeds, so it is tracked, not refetched;
      // an assistant message can't request a skill, so it is skipped.
      // The ledger holds a call as soon as its message.done is stored.
      // A workflow node's calls sit in a child session: the node's end is the cue.
      if ((event.type === 'message.done' && data.stats)
        || event.type === 'workflow.node.finished'
        || event.type === 'workflow.finished') refreshUsage();
      // A round measures the context; a replay cuts the model's view.
      if (event.type === 'context.state' || event.type === 'session.rewound') refreshContext();
      // An image came in, or the model's view was cut: the Vision block's
      // inContext flags can only change then.
      if ((event.type === 'tool.result'
        && String(data.result?.metadata?.mimeType ?? '').startsWith('image/'))
        || (event.type === 'message.start' && data.attachments?.length)
        || event.type === 'context.compacted'
        || event.type === 'session.rewound') {
        refreshVision();
      }
      if ((event.type === 'message.start' && data.role === 'user')
        || event.type === 'context.compacted'
        || event.type === 'session.rewound') {
        refreshSkills();
      } else if (event.type === 'tool.call' && data.toolCall?.name === 'load_skill') {
        // Track the call so its result refetches; the call itself loads nothing.
        loadSkillCallsRef.current.add(String(data.toolCall?.id ?? ''));
      } else if (event.type === 'tool.result' && loadSkillCallsRef.current.has(String(data.toolCallId ?? ''))) {
        refreshSkills();
      }
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
        trackLoadSkillCalls(page, loadSkillCallsRef.current);
        foldAll();
        refreshSkills();
        refreshUsage();
        refreshContext();
        refreshVision();
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
              // The run rail is a convenience; the thread keeps working.
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
  }, [sessionId, foldAll, refreshSkills, refreshUsage, refreshContext, refreshVision]);

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
      trackLoadSkillCalls(page, loadSkillCallsRef.current);
      foldAll();
      // Older pages may hold loads the panel has not seen yet.
      refreshSkills();
      setHasMore(page.length > 0 && page[0].seq > 1);
      return true;
    } catch {
      // The loaded page stays; the button can be tried again.
      return false;
    } finally {
      setLoadingOlder(false);
    }
  }, [sessionId, loadingOlder, foldAll, refreshSkills]);

  return { view, hasMore, loadingOlder, loadOlder, skills, usage, calls, workflowUsage, context, vision };
}
