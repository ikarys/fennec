/**
 * Notifies the human when any session's turn ends, wherever they are in fennec.
 *
 * The per-page notice only fired for the open session; this hook listens to the
 * server-wide stream so a turn can be answered from the home page, settings, or
 * another project.
 */
import { useEffect } from 'react';

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
      if (!document.hidden && window.location.hash.startsWith('#/s/' + notice.sessionId)) return;
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
