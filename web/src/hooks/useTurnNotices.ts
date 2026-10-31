/**
 * Notifies the human when any session's turn ends, wherever they are in fennec.
 *
 * The per-page notice only fired for the open session; this hook listens to the
 * server-wide stream so a turn can be answered from the home page, settings, or
 * another project.
 */
import { useEffect } from 'react';

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

export function useTurnNotices(): void {
  useEffect(() => {
    const source = new EventSource('/api/notices');
    source.onmessage = (event) => {
      const notice = JSON.parse(event.data) as {
        sessionId: string;
        title: string;
        projectId: string;
      };
      if (!('Notification' in window) || Notification.permission !== 'granted') return;
      if (!document.hidden && hashShowsSession(window.location.hash, notice.sessionId)) return;
      const notification = new Notification(notice.title || 'fennec', {
        body: 'Tour terminé : à toi de jouer.',
        tag: notice.sessionId,
      });
      notification.onclick = () => {
        window.focus();
        window.location.hash = '#/s/' + notice.sessionId;
        notification.close();
      };
    };
    return () => source.close();
  }, []);
}
