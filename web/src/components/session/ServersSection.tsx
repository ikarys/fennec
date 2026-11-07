import { useState } from 'react';
import type { ServerView } from '../../lib/api';
import { ServerIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { panelBlock } from '../../lib/panelBlocks';
import { ServerEditor } from './ServerEditor';
import { ServerList } from './ServerList';
import { useT } from '../../i18n';

interface ServersSectionProps {
  sessionId: string | null;
}

export function ServersSection({ sessionId }: ServersSectionProps) {
  const t = useT();
  const [servers, setServers] = useState<ServerView[] | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);

  if (sessionId === null) {
    return null;
  }

  const runningCount = servers?.filter((server) => server.running).length ?? 0;
  const summary = servers === null ? undefined : `${runningCount}/${servers.length}`;

  return (
    <Panel
      id="servers"
      title={t(panelBlock('servers').titleKey)}
      icon={<ServerIcon className="h-3.5 w-3.5" />}
      iconTone={runningCount > 0 ? 'accent' : 'neutral'}
      summary={summary}
    >
      <div className="flex flex-col gap-1.5">
        <p className="m-0 text-xs text-muted">
          {t('serversStartedNote')} <code>$PORT</code>.
        </p>

        <ServerList
          key={refreshKey}
          sessionId={sessionId}
          onServers={setServers}
          onConfigure={() => setEditorOpen(true)}
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
