import { useState } from 'react';
import type { Project } from '../../lib/api';
import { useT } from '../../i18n';
import { SessionList } from './SessionList';

export function ChatSection({
  project,
  onOpenSession,
  selected,
  selecting,
  onToggle,
  onDelete
}: {
  project: Project;
  onOpenSession: (sessionId: string) => void;
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const t = useT();
  const [expanded, setExpanded] = useState(false);
  return (
    <section className="overflow-hidden rounded-md border border-border bg-panel">
      <div className="flex min-h-12 items-center gap-2.5 px-3 py-2">
        <span className="truncate text-sm font-semibold">{t('homeChat')}</span>
        <span className="ml-1 flex min-w-0 gap-3 font-mono text-xs text-faint">
          <span className="truncate whitespace-nowrap tabular-nums">
            {t('homeConversationCount', { count: project.sessions.length })}
          </span>
        </span>
      </div>

      <SessionList
        sessions={project.sessions}
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
