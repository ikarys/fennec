/**
 * The session's servers block: a quiet toolbar, the server list, and the editor.
 */
import { useState } from 'react';
import type { ServerView } from '../../lib/api';
import { InfoIcon, ServerIcon, SettingsIcon } from '../ui/Icons';
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
        <div className="flex items-center justify-end gap-0.5">
          <span role="img" className="text-muted p-1 cursor-help" title={note} aria-label={note}>
            <InfoIcon className="h-3.5 w-3.5" />
          </span>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            onClick={() => setEditorOpen(true)}
            title={t('configure')}
            aria-label={t('configure')}
          >
            <SettingsIcon className="h-3.5 w-3.5" />
          </Button>
        </div>

        <ServerList
          key={refreshKey}
          sessionId={sessionId}
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
