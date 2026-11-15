// The servers' words and refresh pace as the UI shows them. Pure, so tested without a browser.
import type { BadgeTone } from '../components/ui/Badge';
import type { ServerView } from './api';
import { t, type Lang } from '../i18n/messages';


/** A server's state in words, with the tone and pulse of its badge. */
export function serverBadge(
  lang: Lang,
  server: ServerView,
): { text: string; tone: BadgeTone; pulse: boolean } {
  switch (server.state) {
    case 'stopped':
      return { text: t(lang, 'serverStopped'), tone: 'neutral', pulse: false };
    case 'starting':
      return { text: t(lang, 'serverStarting'), tone: 'info', pulse: true };
    case 'ready':
      return { text: t(lang, 'serverReady'), tone: 'ok', pulse: true };
    case 'exited':
      if (server.exitSignal !== null) {
        return {
          text: t(lang, 'serverSignaled', { signal: server.exitSignal }),
          tone: 'attention',
          pulse: false,
        };
      }
      if (server.exitCode === 0) {
        return { text: t(lang, 'serverEnded'), tone: 'neutral', pulse: false };
      }
      if (server.exitCode === null) {
        return { text: t(lang, 'serverExitedNoCode'), tone: 'err', pulse: false };
      }
      return {
        text: t(lang, 'serverExited', { code: server.exitCode }),
        tone: 'err',
        pulse: false,
      };
  }
}

/** The short label for a server URL: its port when present, the URL otherwise. */
export function portLabel(url: string): string {
  try {
    const port = new URL(url).port;
    return port === '' ? url : `:${port}`;
  } catch {
    return url;
  }
}

/** How often the list re-reads the servers: fast while one starts, slower while output moves, never when idle. */
export function serversPollMs(servers: ServerView[] | null): number | null {
  if (servers === null) return null;
  if (servers.some((server) => server.state === 'starting')) return 1000;
  if (servers.some((server) => server.state === 'ready')) return 3000;
  return null;
}
