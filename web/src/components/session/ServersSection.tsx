/**
 * The session's servers block: the server list and its editor.
 */
import { useState } from 'react';
import type { ServerView } from '../../lib/api';
import { PlusIcon, ServerIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Panel } from '../ui/Panel';
import { ServerEditor } from './ServerEditor';
import { ServerList } from './ServerList';
import { useT } from '../../i18n';

interface ServersSectionProps {
  sessionId: string | null;
  onServers?: (servers: ServerView[]) => void;
}

export function ServersSection({ sessionId, onServers }: ServersSectionProps) {
  const t = useT();
  const [servers, setServers] = useState<ServerView[] | null>(null);
  const [editing, setEditing] = useState<{ name: string | null } | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  if (sessionId === null) {
    return null;
  }

  const runningCount = servers?.filter((server) => server.running).length ?? 0;
  const summary = (
    <span className="flex items-center gap-1">
      {servers !== null && `${runningCount}/${servers.length}`}
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        title={t('serverAdd')}
        aria-label={t('serverAdd')}
        onClick={(event) => {
          // Inside the section's summary: adding must not fold the section.
          event.preventDefault();
          setEditing({ name: null });
        }}
      >
        <PlusIcon className="h-3.5 w-3.5" />
      </Button>
    </span>
  );
  const note = `${t('serversStartedNote')} $PORT.`;

  return (
    <Panel
      id="servers"
      title={t('panelServers')}
      icon={<ServerIcon className="h-3.5 w-3.5" />}
      iconTone={runningCount > 0 ? 'accent' : 'neutral'}
      summary={summary}
      hint={note}
    >
      <div className="flex flex-col gap-1.5">
        <ServerList
          key={refreshKey}
          sessionId={sessionId}
          onEdit={(name) => setEditing({ name })}
          onServers={(nextServers) => {
            setServers(nextServers);
            onServers?.(nextServers);
          }}
        />

        <ServerEditor
          open={editing !== null}
          sessionId={sessionId}
          servers={servers ?? []}
          editing={editing?.name ?? null}
          onSaved={() => {
            setEditing(null);
            setRefreshKey((key) => key + 1);
          }}
          onClose={() => setEditing(null)}
        />
      </div>
    </Panel>
  );
}
