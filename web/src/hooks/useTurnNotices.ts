/**
 * The human's attention, server-wide: a snapshot of the sessions that need
 * their eye, kept as state for the in-app badge, and turned into a system
 * notification for what is new, wherever they are in fennec.
 *
 * The per-page notice only fired for the open session; this listens to the
 * server-wide stream so a question, a workflow waiting on a choice, finished
 * work or an error can be answered from the home page, settings, or another
 * project.
 */
import { useEffect, useRef, useState } from 'react';
import { useT } from '../i18n';
import type { MsgKey } from '../i18n/messages';
import type { AttentionItem } from '../lib/api';
import type { BudgetNotice } from '../lib/api/sessions';

/** True when the current hash shows this session: `#/s/<id>` or a split pane `#/split/<a>/<b>`. */
function hashShowsSession(hash: string, sessionId: string): boolean {
  if (hash.startsWith('#/s/' + sessionId)) return true;
  if (hash.startsWith('#/split/')) {
    return hash
      .slice('#/split/'.length)
      .split('/')
      .some((token) => token !== '-' && decodeURIComponent(token) === sessionId);
  }
  return false;
}

function bodyFor(item: AttentionItem, t: (key: MsgKey, vars?: Record<string, string | number>) => string): string {
  switch (item.kind) {
    case 'question':
      return item.detail ? t('turnNoticeQuestionDetail', { detail: item.detail }) : t('turnNoticeQuestion');
    case 'workflowPause':
      return item.detail ? t('turnNoticeWorkflowPauseDetail', { detail: item.detail }) : t('turnNoticeWorkflowPause');
    case 'awaitingIntegration':
      return t('turnNoticeAwaitingIntegration');
    case 'conflict':
      return item.detail ? t('turnNoticeConflictDetail', { detail: item.detail }) : t('turnNoticeConflict');
    case 'error':
      return item.detail ? t('turnNoticeErrorDetail', { detail: item.detail }) : t('turnNoticeError');
  }
}

export function useTurnNotices(onOpen?: (sessionId: string) => void): {
  items: AttentionItem[];
  /** Moves on every accepted plugin block update: the block-fetching components refetch on it. */
  pluginsVersion: number;
  /** False once the server stream has been down past a short grace delay. */
  online: boolean;
} {
  const t = useT();
  const [items, setItems] = useState<AttentionItem[]>([]);
  const [pluginsVersion, setPluginsVersion] = useState(0);
  const [online, setOnline] = useState(true);
  const previous = useRef<Set<string>>(new Set());
  const previousBudgets = useRef<Set<string>>(new Set());
  const baseline = useRef(true);
  // The callback is read through a ref so a notification click always uses the
  // latest `openSession` without re-creating the stream on every render.
  const onOpenRef = useRef(onOpen);
  onOpenRef.current = onOpen;

  useEffect(() => {
    let source: EventSource | null = null;
    let offlineTimer: number | null = null;
    let reconnectTimer: number | null = null;

    const clearOfflineTimer = () => {
      if (offlineTimer === null) return;
      window.clearTimeout(offlineTimer);
      offlineTimer = null;
    };

    // Down past this grace delay, the page says so: a quick restart does not flash it.
    const armOffline = () => {
      if (offlineTimer !== null) return;
      offlineTimer = window.setTimeout(() => {
        offlineTimer = null;
        setOnline(false);
      }, 2000);
    };

    // A stream not open after a while is rebuilt: the browser gives up for good on
    // an error status, and the dev proxy leaves a request to a stopped server hanging.
    const armWatchdog = () => {
      if (reconnectTimer !== null) return;
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null;
        if (source?.readyState !== EventSource.OPEN) connect();
      }, 3000);
    };

    const connect = () => {
      source?.close();
      const nextSource = new EventSource('/api/notices');
      source = nextSource;
      armOffline();
      armWatchdog();

      nextSource.onopen = () => {
        setOnline(true);
        clearOfflineTimer();
      };

      nextSource.onmessage = (event) => {
        setOnline(true);
        clearOfflineTimer();
        let snapshot: { items: AttentionItem[]; budgets?: BudgetNotice[]; pluginsVersion?: number };
        try {
          snapshot = JSON.parse(event.data);
        } catch {
          // A truncated frame (a dropped connection) waits for the next snapshot.
          return;
        }
        const budgets = snapshot.budgets ?? [];
        setItems(snapshot.items);
        setPluginsVersion(snapshot.pluginsVersion ?? 0);
        // The first snapshot is the baseline: it populates the badge but does
        // not notify for what was already waiting before the tab opened.
        const first = baseline.current;
        baseline.current = false;
        if (
          !first &&
          'Notification' in window &&
          Notification.permission === 'granted'
        ) {
          for (const item of snapshot.items) {
            if (previous.current.has(item.sessionId)) continue;
            if (!document.hidden && hashShowsSession(window.location.hash, item.sessionId)) continue;
            const notification = new Notification(item.title || 'fennec', {
              body: bodyFor(item, t),
              tag: item.sessionId,
            });
            notification.onclick = () => {
              window.focus();
              if (onOpenRef.current) {
                onOpenRef.current(item.sessionId);
              } else {
                window.location.hash = '#/s/' + item.sessionId;
              }
              notification.close();
            };
          }
          for (const budget of budgets) {
            const key = budget.projectId + ':' + budget.status;
            if (previousBudgets.current.has(key)) continue;
            const notification = new Notification(budget.project, {
              body: budget.detail,
              tag: 'budget:' + key,
            });
            notification.onclick = () => {
              window.focus();
              window.location.hash = '#/project/' + encodeURIComponent(budget.projectId);
              notification.close();
            };
          }
        }
        previous.current = new Set(snapshot.items.map((item) => item.sessionId));
        previousBudgets.current = new Set(
          budgets.map((budget) => budget.projectId + ':' + budget.status),
        );
      };

      nextSource.onerror = () => {
        armOffline();
        armWatchdog();
      };
    };

    connect();

    return () => {
      source?.close();
      clearOfflineTimer();
      if (reconnectTimer !== null) window.clearTimeout(reconnectTimer);
    };
  }, []);

  return { items, pluginsVersion, online };
}
