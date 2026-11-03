import { useCallback, useEffect, useRef, useState } from 'react';
import type { ServerView } from '../../lib/api';
import { getServers, startServer, stopServer } from '../../lib/api';
import { ExternalIcon, PlayIcon, SettingsIcon, SpinnerIcon, StopIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';
import { Panel } from '../ui/Panel';
import { panelBlock } from '../../lib/panelBlocks';
import { ServerEditor } from './ServerEditor';

interface ServersSectionProps {
  sessionId: string | null;
}

export function ServersSection({ sessionId }: ServersSectionProps) {
  const [servers, setServers] = useState<ServerView[] | null>(null);
  const [busyName, setBusyName] = useState<string | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const fetchId = useRef(0);

  const fetchServers = useCallback(async () => {
    if (!sessionId) {
      fetchId.current += 1;
      setServers(null);
      return;
    }
    const id = ++fetchId.current;
    try {
      const data = await getServers(sessionId);
      if (id === fetchId.current) setServers(data);
    } catch {
      if (id === fetchId.current) setServers([]);
    }
  }, [sessionId]);

  useEffect(() => {
    void fetchServers();
  }, [fetchServers]);

  const toggle = async (server: ServerView) => {
    if (!sessionId || busyName !== null) return;
    setBusyName(server.name);
    try {
      if (server.running) await stopServer(sessionId, server.name);
      else await startServer(sessionId, server.name);
      await fetchServers();
    } catch {
      // The state is re-read on the next action; a failed start shows as stopped.
    } finally {
      setBusyName(null);
    }
  };

  if (sessionId === null) {
    return null;
  }

  const runningCount = servers?.filter((s) => s.running).length ?? 0;
  const summary = servers === null ? undefined : `${runningCount}/${servers.length}`;

  return (
    <Panel id="servers" title={panelBlock('servers').title} summary={summary}>
      <div className="flex flex-col gap-1.5">
        <p className="m-0 text-xs text-muted">
          Démarrés dans le worktree de la session, sur le port <code>$PORT</code>.
        </p>
        {servers === null && <div className="text-xs text-muted">Chargement…</div>}
        {servers !== null && servers.length === 0 && (
          <div className="text-xs text-muted">Aucun serveur configuré.</div>
        )}
        {servers?.map((server) => (
          <div key={server.name} className="flex items-center gap-2 rounded-md px-1 py-1 hover:bg-accent-soft">
            <Badge tone={server.running ? 'ok' : 'neutral'} dot pulse={server.running}>
              {server.running ? 'en cours' : 'arrêt'}
            </Badge>
            <span className="min-w-0 grow truncate font-mono text-xs" title={server.command}>
              {server.name}
            </span>
            {server.url !== null && (
              <a
                href={server.url}
                target="_blank"
                rel="noreferrer"
                className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
                aria-label={`Ouvrir ${server.url}`}
                title={`Ouvrir ${server.url}`}
              >
                <ExternalIcon className="h-3.5 w-3.5" />
              </a>
            )}
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              disabled={busyName !== null}
              aria-label={server.running ? `Arrêter ${server.name}` : `Démarrer ${server.name}`}
              title={server.command}
              onClick={() => void toggle(server)}
            >
              {busyName === server.name ? (
                <SpinnerIcon className="h-3.5 w-3.5" />
              ) : server.running ? (
                <StopIcon className="h-3.5 w-3.5" />
              ) : (
                <PlayIcon className="h-3.5 w-3.5" />
              )}
            </Button>
          </div>
        ))}
        <Button variant="soft" size="sm" onClick={() => setEditorOpen(true)}>
          <SettingsIcon className="h-3.5 w-3.5" />
          Configurer
        </Button>
      </div>
      <ServerEditor
        open={editorOpen}
        sessionId={sessionId}
        initial={servers ?? []}
        onSaved={() => {
          setEditorOpen(false);
          void fetchServers();
        }}
        onClose={() => setEditorOpen(false)}
      />
    </Panel>
  );
}
