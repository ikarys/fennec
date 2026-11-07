// The project's servers in a session's worktree: their state, output and URL, a terminal there.
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ServerView } from '../../lib/api';
import { getServers, openTerminal, saveServers, startServer, stopServer } from '../../lib/api';
import { serverBadge, serversPollMs } from '../../lib/servers';
import { ExternalIcon, PlayIcon, SettingsIcon, SpinnerIcon, StopIcon, TerminalIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';
import { useLang, useT } from '../../i18n';

function ServerOutput({ lines }: { lines: string[] }) {
  const ref = useRef<HTMLPreElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  return (
    <pre
      ref={ref}
      className="max-h-40 overflow-auto rounded-md border border-border bg-raised p-2 font-mono text-[11px] leading-snug whitespace-pre-wrap break-all"
    >
      {lines.join('\n')}
    </pre>
  );
}

interface ServerListProps {
  sessionId: string;
  onConfigure?: () => void;
  onServers?: (servers: ServerView[]) => void;
}

export function ServerList({ sessionId, onConfigure, onServers }: ServerListProps) {
  const t = useT();
  const lang = useLang();
  const [servers, setServers] = useState<ServerView[] | null>(null);
  const [busyName, setBusyName] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fetchId = useRef(0);
  const onServersRef = useRef(onServers);
  const tRef = useRef(t);

  useEffect(() => {
    onServersRef.current = onServers;
  }, [onServers]);

  useEffect(() => {
    tRef.current = t;
  }, [t]);

  const fetchServers = useCallback(async () => {
    if (!sessionId) {
      fetchId.current += 1;
      setServers(null);
      onServersRef.current?.([]);
      return;
    }

    const id = ++fetchId.current;
    try {
      const data = await getServers(sessionId);
      if (id === fetchId.current) {
        setServers(data);
        onServersRef.current?.(data);
      }
    } catch {
      if (id === fetchId.current) {
        setServers([]);
        onServersRef.current?.([]);
        setError(tRef.current('error'));
      }
    }
  }, [sessionId]);

  useEffect(() => {
    void fetchServers();
  }, [fetchServers]);

  useEffect(() => {
    if (servers === null) return;

    const ms = serversPollMs(servers);
    if (ms === null) return;

    const timer = window.setTimeout(() => {
      void fetchServers();
    }, ms);

    return () => window.clearTimeout(timer);
  }, [servers, fetchServers]);

  const toggle = async (server: ServerView) => {
    if (busyName !== null) return;

    setBusyName(server.name);
    setError(null);

    try {
      if (server.running) await stopServer(sessionId, server.name);
      else await startServer(sessionId, server.name);
      await fetchServers();
    } catch (failure) {
      // The server's reason (a port taken, a command missing) is what the human needs.
      setError(failure instanceof Error ? failure.message : tRef.current('error'));
    } finally {
      setBusyName(null);
    }
  };

  const adoptSuggested = async (server: ServerView) => {
    if (busyName !== null) return;

    setBusyName(server.name);
    setError(null);

    try {
      await saveServers(sessionId, [{ name: server.name, command: server.command }]);
      await startServer(sessionId, server.name);
      await fetchServers();
    } catch (failure) {
      // The server's reason (a port taken, a command missing) is what the human needs.
      setError(failure instanceof Error ? failure.message : tRef.current('error'));
    } finally {
      setBusyName(null);
    }
  };

  const openTerminalHere = async () => {
    setError(null);

    try {
      await openTerminal(sessionId);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : tRef.current('error'));
    }
  };

  return (
    <div className="flex flex-col gap-1.5">
      {servers === null && <div className="text-xs text-muted">{t('loading')}</div>}

      {servers !== null && servers.length === 0 && (
        <div className="text-xs text-muted">{t('noServersConfigured')}</div>
      )}

      {servers?.map((server) => {
        const badge = serverBadge(lang, server);

        return (
          <div key={server.name} className="flex flex-col gap-1.5">
            <div className="flex items-center gap-2 rounded-md px-1 py-1 hover:bg-accent-soft">
              <Badge tone={badge.tone} dot pulse={badge.pulse}>
                {badge.text}
              </Badge>

              <span className="min-w-0 grow truncate font-mono text-xs" title={server.command}>
                {server.suggested ? server.command : server.name}
              </span>

              {server.url !== null && server.state === 'ready' && (
                <a
                  href={server.url}
                  target="_blank"
                  rel="noreferrer"
                  className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
                  aria-label={t('openUrl', { url: server.url })}
                  title={t('openUrl', { url: server.url })}
                >
                  <ExternalIcon className="h-3.5 w-3.5" />
                </a>
              )}

              {server.suggested ? (
                <Button
                  variant="primary"
                  size="sm"
                  disabled={busyName !== null}
                  onClick={() => void adoptSuggested(server)}
                >
                  {busyName === server.name && <SpinnerIcon className="h-3.5 w-3.5" />}
                  {t('useThisCommand')}
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  iconOnly
                  disabled={busyName !== null}
                  aria-label={
                    server.running
                      ? t('stopServer', { name: server.name })
                      : t('startServer', { name: server.name })
                  }
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
              )}
            </div>

            {server.suggested && (
              <p className="m-0 text-[11px] text-muted">{t('suggestedCommand')}</p>
            )}

            {server.output.length > 0 && server.state !== 'stopped' && (
              <ServerOutput lines={server.output} />
            )}
          </div>
        );
      })}

      {error !== null && <div className="text-xs text-err">{error}</div>}

      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('openTerminalInWorktree')}
          aria-label={t('openTerminalInWorktree')}
          onClick={() => void openTerminalHere()}
        >
          <TerminalIcon className="h-3.5 w-3.5" />
        </Button>

        {onConfigure && (
          <Button variant="soft" size="sm" onClick={onConfigure}>
            <SettingsIcon className="h-3.5 w-3.5" />
            {t('configure')}
          </Button>
        )}
      </div>
    </div>
  );
}
