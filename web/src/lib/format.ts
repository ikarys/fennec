import type { SessionState } from './api';

export function formatRelative(ms: number): string {
  const diff = Date.now() - ms;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "à l'instant";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `il y a ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `il y a ${hours} h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `il y a ${days} j`;
  return new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' }).format(new Date(ms));
}

export function formatDuration(ms?: number): string {
  if (ms == null) return '';
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1).replace('.', ',')} s`;
  if (ms >= 3_600_000) {
    const totalMinutes = Math.floor(ms / 60_000);
    return `${Math.floor(totalMinutes / 60)} h ${totalMinutes % 60} min`;
  }
  const seconds = Math.round(ms / 1000);
  return `${Math.floor(seconds / 60)} min ${seconds % 60} s`;
}

/** A wall-clock timestamp as a local time (HH:MM), with a short date if not today. */
export function formatClockTime(ms: number): string {
  const date = new Date(ms);
  const time = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' }).format(date);
  if (new Date().toDateString() === date.toDateString()) return time;
  const day = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' }).format(date);
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

export function formatTokens(value?: number): string {
  if (value == null) return '0';
  // Same scale as the server's token_usage::display::tokens.
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1).replace('.', ',')}M`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(1).replace('.', ',')}k`;
  }
  return String(value);
}

export function stateLabel(state: SessionState): string {
  switch (state) {
    case 'running':
      return 'En cours';
    case 'complete':
      return 'Terminée';
    case 'stopped':
      return 'Arrêtée';
    case 'error':
      return 'Erreur';
    case 'truncated':
      return 'Tronquée';
    case 'idle':
      return 'Inactive';
    case 'waiting_for_user':
      return 'En attente de toi';
  }
}

export function formatSpeed(tokensPerSecond: number): string {
  return `${Math.round(tokensPerSecond)} tok/s`;
}

/** Share of the prompt the server took from its cache, in percent. */
export function cachePercent(cached: number, prompt: number): number {
  return prompt > 0 ? Math.round((cached / prompt) * 100) : 0;
}
