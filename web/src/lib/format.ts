import type { SessionState } from './api';
import { t, type Lang } from '../i18n/messages';

const localeOf = (lang: Lang) => (lang === 'fr' ? 'fr-FR' : 'en-US');

// French writes its decimal comma; the rest of the world a dot.
const decimal = (lang: Lang, text: string) => (lang === 'fr' ? text.replace('.', ',') : text);

export function formatRelative(lang: Lang, ms: number): string {
  const diff = Date.now() - ms;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return t(lang, 'justNow');
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t(lang, 'minutesAgo', { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t(lang, 'hoursAgo', { count: hours });
  const days = Math.floor(hours / 24);
  if (days < 7) return t(lang, 'daysAgo', { count: days });
  return new Intl.DateTimeFormat(localeOf(lang), { day: 'numeric', month: 'short' }).format(new Date(ms));
}

/** A calendar date as YYYY/MM/DD, local time. */
export function formatDate(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())}`;
}

/** A creation timestamp as a full local date + time, always showing the date (2026/10/25 · 23:33). */
export function formatDateTime(lang: Lang, ms: number): string {
  const time = new Intl.DateTimeFormat(localeOf(lang), { hour: '2-digit', minute: '2-digit' }).format(new Date(ms));
  return `${formatDate(ms)} · ${time}`;
}

export function formatDuration(lang: Lang, ms?: number): string {
  if (ms == null) return '';
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${decimal(lang, (ms / 1000).toFixed(1))} s`;
  if (ms >= 3_600_000) {
    const totalMinutes = Math.floor(ms / 60_000);
    return `${Math.floor(totalMinutes / 60)} h ${totalMinutes % 60} min`;
  }
  const seconds = Math.round(ms / 1000);
  return `${Math.floor(seconds / 60)} min ${seconds % 60} s`;
}

/** A wall-clock timestamp as a local time (HH:MM), with a short date if not today. */
export function formatClockTime(lang: Lang, ms: number): string {
  const date = new Date(ms);
  const time = new Intl.DateTimeFormat(localeOf(lang), { hour: '2-digit', minute: '2-digit' }).format(date);
  if (new Date().toDateString() === date.toDateString()) return time;
  const day = new Intl.DateTimeFormat(localeOf(lang), { day: 'numeric', month: 'short' }).format(date);
  return `${time} · ${day}`;
}

export function formatClock(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  if (hours > 0) {
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
}

export function formatBytes(lang: Lang, bytes: number): string {
  if (bytes < 1024) return t(lang, 'bytes', { count: Math.floor(bytes) });

  const units = lang === 'fr' ? ['Ko', 'Mo', 'Go', 'To'] : ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  const text = value < 10 ? decimal(lang, value.toFixed(1)) : String(Math.round(value));
  return `${text} ${units[unit]}`;
}

export function formatTokens(lang: Lang, value?: number): string {
  if (value == null) return '0';
  // Same scale as the server's token_usage::display::tokens.
  if (value >= 1_000_000) {
    return `${decimal(lang, (value / 1_000_000).toFixed(1))}M`;
  }
  if (value >= 1000) {
    return `${decimal(lang, (value / 1000).toFixed(1))}k`;
  }
  return String(value);
}

export function stateLabel(lang: Lang, state: SessionState): string {
  switch (state) {
    case 'running':
      return t(lang, 'running');
    case 'complete':
      return t(lang, 'complete');
    case 'stopped':
      return t(lang, 'stopped');
    case 'error':
      return t(lang, 'error');
    case 'truncated':
      return t(lang, 'truncated');
    case 'idle':
      return t(lang, 'idle');
    case 'waiting_for_user':
      return t(lang, 'waitingForYou');
  }
}

export function formatSpeed(tokensPerSecond: number): string {
  return `${Math.round(tokensPerSecond)} tok/s`;
}

/** Share of the prompt the server took from its cache, in percent. */
export function cachePercent(cached: number, prompt: number): number {
  return prompt > 0 ? Math.round((cached / prompt) * 100) : 0;
}
