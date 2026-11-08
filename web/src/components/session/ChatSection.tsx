import { useState } from 'react';
import type { Project } from '../../lib/api';
import { useT } from '../../i18n';
import { PlusIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { SessionList } from './SessionList';

export function ChatSection({
  project,
  onNewSession,
  onOpenSession,
  selected,
  selecting,
  onToggle,
  onDelete
}: {
  project: Project;
  onNewSession: () => void;
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
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <Button
            variant="ghost"
            size="sm"
            className="text-accent hover:bg-accent-soft hover:text-accent-strong"
            onClick={onNewSession}
            title={t('homeNewChatTitle')}
            aria-label={t('paletteNewChat')}
          >
            <PlusIcon className="h-3.5 w-3.5" />
            {t('paletteNewChat')}
          </Button>
        </div>
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
