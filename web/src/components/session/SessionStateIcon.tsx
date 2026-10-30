/** A session's state as a coloured glyph, its label in the tooltip: a list reads by colour, not by words. */
import type { SessionState } from '../../lib/api';
import { stateLabel } from '../../lib/format';

// Filled discs carry a mark cut out in the panel colour; idle stays a hollow ring.
const DISC: Partial<Record<SessionState, { color: string; mark: string }>> = {
  complete: { color: 'text-ok', mark: 'M8 12.5l2.5 2.5L16 9.5' },
  error: { color: 'text-err', mark: 'M12 7.5v5.5M12 16.5h.01' },
  waiting_for_user: { color: 'text-accent', mark: 'M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .8-1 1.5M12 16.5h.01' },
  stopped: { color: 'text-muted', mark: 'M9.5 9.5h5v5h-5z' },
  truncated: { color: 'text-muted', mark: 'M8 12h.01M12 12h.01M16 12h.01' },
};

export function SessionStateIcon({ state, className = 'h-4 w-4' }: { state: SessionState; className?: string }) {
  const label = stateLabel(state);
  if (state === 'running') {
    return (
      <span title={label} aria-label={label} role="img" className={`inline-flex shrink-0 items-center justify-center ${className}`}>
        <span className="h-2.5 w-2.5 animate-pulse rounded-full bg-accent" />
      </span>
    );
  }
  const disc = DISC[state];
  return (
    <svg className={`shrink-0 ${disc?.color ?? 'text-faint'} ${className}`} viewBox="0 0 24 24" role="img" aria-label={label}>
      <title>{label}</title>
      {disc ? (
        <>
          <circle cx="12" cy="12" r="10" fill="currentColor" />
          <path d={disc.mark} fill="none" stroke="var(--color-panel)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
        </>
      ) : (
        <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2" />
      )}
    </svg>
  );
}
