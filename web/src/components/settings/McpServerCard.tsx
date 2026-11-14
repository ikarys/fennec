// One MCP server accordion row: connection, usage, tools, and delete/save footer.
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { SectionLabel } from '../ui/SectionLabel';
import { SettingRow } from '../ui/SettingRow';
import { Switch } from '../ui/Switch';
import { Segmented } from '../ui/Segmented';
import { TextInput } from '../ui/Field';
import { ChevronRightIcon, KeyIcon, RefreshIcon, SpinnerIcon, TrashIcon } from '../ui/Icons';
import { enabledToolStats, McpTools } from './McpTools';
import type { McpServer } from '../../lib/api';
import type { McpToolSummary } from '../../lib/api/settings';
import { useLang, useT } from '../../i18n';
import { formatTokens } from '../../lib/format';

export function McpServerCard({
  index,
  server,
  open,
  onToggle,
  connecting,
  views,
  saveState,
  actionError,
  onPatch,
  onRemove,
  onConnect,
  onAuthorize
}: {
  index: number;
  server: McpServer;
  open: boolean;
  onToggle: () => void;
  connecting: boolean;
  views: McpToolSummary[];
  saveState: { saving: boolean; error: string | null };
  /** Why the last connect or authorize failed, shown next to its buttons. */
  actionError: string | null;
  onPatch: (patch: Partial<McpServer>) => void;
  onRemove: () => void;
  onConnect: () => void;
  onAuthorize: () => void;
}) {
  const t = useT();
  const lang = useLang();
  const stats = enabledToolStats(server, views);
  const bodyId = `mcp-${index}-body`;
  const nameId = `mcp-${index}-name`;
  const urlId = `mcp-${index}-url`;
  const commandId = `mcp-${index}-command`;
  const argsId = `mcp-${index}-args`;
  const tokenId = `mcp-${index}-token`;
  const enabled = !server.disabled;

  return (
    <div className="overflow-hidden rounded-xl border border-border bg-panel hover:border-accent/40">
      <div className={`flex items-center gap-3 px-4 py-3 ${open ? 'border-b border-border bg-accent/5' : ''}`}>
        <button
          type="button"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={onToggle}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <ChevronRightIcon className={`h-4 w-4 shrink-0 text-muted transition-transform ${open ? 'rotate-90' : ''}`} />
          <span className={`truncate font-mono text-sm ${server.name === '' ? 'text-faint' : 'text-text'}`}>
            {server.name === '' ? t('mcpNewServer') : server.name}
          </span>
          <span title={server.transport === 'http' ? t('mcpRemoteHint') : t('mcpLocalHint')}>
            <Badge appearance="pill" tone="info">
              {server.transport === 'http' ? t('mcpRemote') : t('mcpLocal')}
            </Badge>
          </span>
          <span className={`ml-auto shrink-0 text-xs ${server.cachedTools.length > 0 ? 'text-muted' : 'text-faint'}`}>
            {server.cachedTools.length > 0
              ? `${t('mcpToolCount', { count: stats.total })} · ${t('mcpTokens', {
                  count: formatTokens(lang, stats.tokens)
                })}`
              : t('mcpNotConnected')}
          </span>
        </button>
        <div className="flex shrink-0 items-center gap-2">
          <span className={`text-xs ${enabled ? 'text-ok' : 'text-muted'}`}>{enabled ? t('mcpOn') : t('mcpOff')}</span>
          <Switch checked={enabled} onChange={(on) => onPatch({ disabled: !on })} label={t('mcpEnabledLabel')} />
        </div>
      </div>

      {open ? (
        <div id={bodyId}>
          <div className="px-4 py-4">
            <SectionLabel>{t('mcpConnection')}</SectionLabel>
            <p className="m-0 mt-1 mb-3 text-xs text-muted">{t('mcpConnectionHint')}</p>
            <div className="rounded-md bg-raised px-3 py-2">
              <div className="flex items-center justify-between gap-3">
                <span className="text-sm text-text">
                  {server.cachedTools.length > 0 ? t('mcpToolCount', { count: stats.total }) : t('mcpNotConnected')}
                </span>
                <div className="flex shrink-0 items-center gap-2">
                  {/* A token means Bearer auth: OAuth is only offered to a server without one. */}
                  {server.transport === 'http' && !server.authToken ? (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={onAuthorize}
                      title={t('mcpAuthorize')}
                      disabled={connecting || server.name === ''}
                    >
                      <KeyIcon className="h-3.5 w-3.5" />
                      {t('mcpAuthorizeShort')}
                    </Button>
                  ) : null}
                  <Button variant="primary" size="sm" onClick={onConnect} disabled={connecting || server.name === ''}>
                    {connecting ? <SpinnerIcon className="h-3.5 w-3.5" /> : <RefreshIcon className="h-3.5 w-3.5" />}
                    {connecting
                      ? t('mcpConnecting')
                      : server.cachedTools.length > 0
                        ? t('mcpReconnect')
                        : t('mcpConnect')}
                  </Button>
                </div>
              </div>
              {actionError ? <p className="m-0 mt-2 text-xs text-err">{actionError}</p> : null}
            </div>
            <div className="mt-3 grid grid-cols-[140px_1fr] items-center gap-x-4 gap-y-3">
              <label htmlFor={nameId} className="text-xs font-medium text-muted">
                {t('mcpNameLabel')}
              </label>
              <div>
                <TextInput
                  id={nameId}
                  value={server.name}
                  onChange={(event) => onPatch({ name: event.target.value })}
                  placeholder={t('mcpNamePlaceholder')}
                  className="w-full font-mono"
                />
                <p className="m-0 mt-1 text-xs text-faint">{t('mcpNameHint')}</p>
              </div>

              <span className="text-xs font-medium text-muted">{t('mcpTransportLabel')}</span>
              <div className="flex">
                <Segmented
                  label={t('mcpTransportLabel')}
                  value={server.transport}
                  onChange={(v) => onPatch({ transport: v as McpServer['transport'] })}
                  segments={[
                    { value: 'http', label: t('mcpRemoteUrl') },
                    { value: 'stdio', label: t('mcpLocalCommand') }
                  ]}
                />
              </div>

              {server.transport === 'http' ? (
                <>
                  <label htmlFor={urlId} className="text-xs font-medium text-muted">
                    {t('mcpUrlLabel')}
                  </label>
                  <TextInput
                    id={urlId}
                    value={server.url ?? ''}
                    onChange={(event) => onPatch({ url: event.target.value })}
                    placeholder={t('mcpUrlPlaceholder')}
                    className="w-full font-mono"
                  />
                </>
              ) : (
                <>
                  <label htmlFor={commandId} className="text-xs font-medium text-muted">
                    {t('mcpCommandLabel')}
                  </label>
                  <TextInput
                    id={commandId}
                    value={server.command ?? ''}
                    onChange={(event) => onPatch({ command: event.target.value })}
                    placeholder={t('mcpCommandPlaceholder')}
                    className="w-full font-mono"
                  />
                  <label htmlFor={argsId} className="text-xs font-medium text-muted">
                    {t('mcpArgsLabel')}
                  </label>
                  <TextInput
                    id={argsId}
                    value={server.args.join(' ')}
                    onChange={(event) =>
                      onPatch({
                        args: event.target.value.split(/\s+/).filter((part) => part !== '')
                      })
                    }
                    placeholder={t('mcpArgsPlaceholder')}
                    className="w-full font-mono"
                  />
                </>
              )}

              <label htmlFor={tokenId} className="text-xs font-medium text-muted">
                {t('mcpTokenLabel')}
              </label>
              <div>
                <TextInput
                  id={tokenId}
                  type="password"
                  autoComplete="off"
                  value={server.authToken ?? ''}
                  onChange={(event) => onPatch({ authToken: event.target.value || null })}
                  placeholder={t('mcpAuthTokenPlaceholder')}
                  className="w-full font-mono"
                />
                <p className="m-0 mt-1 text-xs text-faint">
                  {server.transport === 'http' ? t('mcpAuthTokenBearer') : t('mcpAuthTokenEnv')}
                </p>
              </div>
            </div>
          </div>

          <div className="border-t border-border px-4 py-4">
            <SectionLabel>{t('mcpUsage')}</SectionLabel>
            <p className="m-0 mt-1 mb-3 text-xs text-muted">{t('mcpUsageHint')}</p>
            <div className="rounded-md border border-border">
              <SettingRow label={t('mcpEnabledLabel')} hint={t('mcpEnabledHint')}>
                <Switch checked={enabled} onChange={(on) => onPatch({ disabled: !on })} label={t('mcpEnabledLabel')} />
              </SettingRow>
              <SettingRow label={t('mcpShownLabel')} hint={t('mcpShownHint')}>
                <Switch
                  checked={server.visible}
                  onChange={(on) => onPatch({ visible: on })}
                  label={t('mcpShownLabel')}
                />
              </SettingRow>
            </div>
          </div>

          {server.cachedTools.length > 0 ? (
            <div className="border-t border-border px-4 py-4">
              <SectionLabel>{t('mcpTools')}</SectionLabel>
              <p className="m-0 mt-1 mb-3 text-xs text-muted">
                {stats.enabled} / {stats.total} {t('mcpToolsActive')} ·{' '}
                {t('mcpTokens', { count: formatTokens(lang, stats.tokens) })} {t('mcpToolsSent')}
              </p>
              <McpTools server={server} views={views} onPatch={onPatch} />
            </div>
          ) : null}

          <div className="flex items-center justify-between border-t border-border px-4 py-3">
            <Button variant="danger" size="sm" onClick={onRemove}>
              <TrashIcon className="h-3.5 w-3.5" />
              {t('mcpRemoveServer')}
            </Button>
            <p className={`m-0 min-h-4 text-xs ${saveState.error ? 'text-err' : 'text-muted'}`}>
              {saveState.error ?? (saveState.saving ? t('sectionSaving') : null)}
            </p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
