/**
 * The session's servers block: the server list and its editor.
 */
import { useState } from 'react';
import type { ServerView } from '../../lib/api';
import { ServerIcon } from '../ui/Icons';
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
  const [editorOpen, setEditorOpen] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);

  if (sessionId === null) {
    return null;
  }

  const runningCount = servers?.filter((server) => server.running).length ?? 0;
  const summary = servers === null ? undefined : `${runningCount}/${servers.length}`;
  const note = `${t('serversStartedNote')} $PORT.`;

  return (
    <Panel
      id="servers"
      title={t('panelServers')}
      icon={<ServerIcon className="h-3.5 w-3.5" />}
      iconTone={runningCount > 0 ? 'accent' : 'neutral'}
      summary={summary}
    >
      <div className="flex flex-col gap-1.5">
        <ServerList
          key={refreshKey}
          sessionId={sessionId}
          note={note}
          onConfigure={() => setEditorOpen(true)}
          onServers={(nextServers) => {
            setServers(nextServers);
            onServers?.(nextServers);
          }}
        />

        <ServerEditor
          open={editorOpen}
          sessionId={sessionId}
          initial={servers ?? []}
          onSaved={() => {
            setEditorOpen(false);
            setRefreshKey((key) => key + 1);
          }}
          onClose={() => setEditorOpen(false)}
        />
      </div>
    </Panel>
  );
}
