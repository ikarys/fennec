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

export function useTurnNotices(onOpen?: (sessionId: string) => void): { items: AttentionItem[] } {
  const t = useT();
  const [items, setItems] = useState<AttentionItem[]>([]);
  const previous = useRef<Set<string>>(new Set());
  const baseline = useRef(true);
  // The callback is read through a ref so a notification click always uses the
  // latest `openSession` without re-creating the stream on every render.
  const onOpenRef = useRef(onOpen);
  onOpenRef.current = onOpen;

  useEffect(() => {
    const source = new EventSource('/api/notices');
    source.onmessage = (event) => {
      const snapshot = JSON.parse(event.data) as { items: AttentionItem[] };
      setItems(snapshot.items);
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
      }
      previous.current = new Set(snapshot.items.map((item) => item.sessionId));
    };
    return () => source.close();
  }, []);

  return { items };
}
