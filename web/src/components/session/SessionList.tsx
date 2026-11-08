import type { SessionSummary } from '../../lib/api';
import { formatRelative, formatDateTime } from '../../lib/format';
import { SessionStateIcon } from './SessionStateIcon';
import { useLang, useT } from '../../i18n';
import { TrashIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { SectionLabel } from '../ui/SectionLabel';

/** How many non-active sessions a project section shows before the toggle appears. */
const RECENT_SESSIONS = 5;

/** The states a session card keeps visible even when the list is collapsed. */
const ACTIVE_STATES: Set<string> = new Set(['running', 'waiting_for_user', 'error']);

function SessionRow({
  session,
  selected,
  selecting,
  onToggle,
  onDelete,
  onOpenSession
}: {
  session: SessionSummary;
  selected: boolean;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenSession: (id: string) => void;
}) {
  const t = useT();
  const lang = useLang();
  const running = session.state === 'running';
  // The checkbox stays hidden until the row is hovered/focused or a selection exists.
  const hidden = selecting || selected ? '' : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100';

  return (
    <div className="group flex h-[34px] w-full items-center gap-3 border-t border-border px-3 hover:bg-raised">
      <input
        type="checkbox"
        className={`h-3.5 w-3.5 shrink-0 ${hidden}`}
        aria-label={t('selectSessionAria', { title: session.title })}
        checked={selected}
        disabled={running}
        title={running ? t('runningNotDeletable') : undefined}
        onChange={() => onToggle(session.id)}
      />
      <button
        type="button"
        onClick={() => onOpenSession(session.id)}
        className="flex min-w-0 grow items-center gap-3 text-left"
      >
        <SessionStateIcon state={session.state} />
        <span className="min-w-0 grow truncate text-sm text-text">{session.title}</span>
        {session.awaitingIntegration && <Badge tone="accent" appearance="solid">{t('toIntegrateBadge')}</Badge>}
        <span title={formatDateTime(lang, session.createdAt)} className="w-24 shrink-0 whitespace-nowrap text-right font-mono text-xs text-faint tabular-nums">
          {formatRelative(lang, session.updatedAt)}
        </span>
      </button>
      {!running && (
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-label={t('deleteSession')}
          title={t('deleteSession')}
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          onClick={() => onDelete(session.id)}
        >
          <TrashIcon className="h-3.5 w-3.5" />
        </Button>
      )}
    </div>
  );
}

/** The shared session list: the active/recent split, the expand toggle and the empty state. */
export function SessionList({
  sessions,
  selected,
  selecting,
  onToggle,
  onDelete,
  onOpenSession,
  empty,
  noun,
  expanded,
  onExpandedChange
}: {
  sessions: SessionSummary[];
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenSession: (id: string) => void;
  empty: string;
  noun: string;
  expanded: boolean;
  onExpandedChange: (value: boolean) => void;
}) {
  const t = useT();
  // The server already returns the sessions newest first.
  const active = sessions.filter((session) => ACTIVE_STATES.has(session.state));
  const rest = sessions.filter((session) => !ACTIVE_STATES.has(session.state));
  const recent = rest.slice(0, RECENT_SESSIONS);
  const hidden = rest.slice(RECENT_SESSIONS);

  if (sessions.length === 0) {
    return (
      <div className="border-t border-border px-3 py-2.5 text-sm text-muted">{empty}</div>
    );
  }

  return (
    <div>
      {active.length > 0 && <SectionLabel className="px-3 pt-2 pb-1">{t('running')}</SectionLabel>}
      {active.map((session) => (
        <SessionRow
          key={session.id}
          session={session}
          selected={selected.has(session.id)}
          selecting={selecting}
          onToggle={onToggle}
          onDelete={onDelete}
          onOpenSession={onOpenSession}
        />
      ))}
      {active.length > 0 && recent.length > 0 && (
        <SectionLabel className="px-3 pt-2 pb-1">{t('homeRecent')}</SectionLabel>
      )}
      {recent.map((session) => (
        <SessionRow
          key={session.id}
          session={session}
          selected={selected.has(session.id)}
          selecting={selecting}
          onToggle={onToggle}
          onDelete={onDelete}
          onOpenSession={onOpenSession}
        />
      ))}
      {rest.length > RECENT_SESSIONS && (
        <button
          type="button"
          onClick={() => onExpandedChange(!expanded)}
          className="block w-full border-t border-border px-3 py-[7px] text-left text-xs text-faint hover:bg-raised hover:text-text"
        >
          {expanded ? t('collapse') : t('homeShowOther', { count: hidden.length, noun })}
        </button>
      )}
      {expanded &&
        hidden.map((session) => (
          <SessionRow
            key={session.id}
            session={session}
            selected={selected.has(session.id)}
            selecting={selecting}
            onToggle={onToggle}
            onDelete={onDelete}
            onOpenSession={onOpenSession}
          />
        ))}
    </div>
  );
}
