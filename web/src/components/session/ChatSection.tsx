/** Home chat section: conversations with no project. */
import { useState } from 'react';
import type { SessionSummary } from '../../lib/api';
import { useT } from '../../i18n';
import { ChatIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { SessionList } from './SessionList';

export function ChatSection({
  title,
  sessions,
  onOpenSession,
  selected,
  selecting,
  onToggle,
  onDelete
}: {
  title: string;
  sessions: SessionSummary[];
  onOpenSession: (sessionId: string) => void;
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const t = useT();
  const [expanded, setExpanded] = useState(false);
  return (
    <section className="overflow-hidden rounded-xl border border-border bg-panel">
      <div className="flex min-h-11 items-center gap-2.5 bg-linear-to-r from-accent-soft to-transparent to-75% px-3 py-2">
        <span className="grid size-6 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
          <ChatIcon className="h-3.5 w-3.5" />
        </span>
        <span className="min-w-0 grow truncate text-sm font-semibold">{title}</span>
        <Badge tone="neutral" appearance="pill" title={t('homeConversationCount', { count: sessions.length })}>
          {sessions.length}
        </Badge>
      </div>

      <SessionList
        sessions={sessions}
        selected={selected}
        selecting={selecting}
        onToggle={onToggle}
        onDelete={onDelete}
        onOpenSession={onOpenSession}
        empty={t('homeNoConversations')}
        noun={t('homeConversationsNoun')}
        expanded={expanded}
        onExpandedChange={setExpanded}
      />
    </section>
  );
}
