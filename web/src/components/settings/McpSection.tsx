import { useState } from 'react';
import { Section, type SectionSave } from '../ui/Section';
import { TextInput, Select } from '../ui/Field';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { ListCard } from '../ui/ListCard';
import { Tabs } from '../ui/Tabs';
import { authorizeMcp, connectMcp, type McpServer } from '../../lib/api';
import type { McpToolSummary, ToolAccess } from '../../lib/api/settings';
import { KeyIcon } from '../Icons';
import { useLang, useT } from '../../i18n';
import { formatTokens } from '../../lib/format';

function emptyServer(): McpServer {
  return {
    name: '',
    transport: 'stdio',
    command: '',
    args: [],
    env: {},
    url: '',
    headers: {},
    disabled: false,
    visible: true,
    disabledTools: [],
    cachedTools: [],
    timeout: 0
  };
}

/** The description's first sentence, for the one-line summary of a tool row. */
function firstSentence(text: string): string {
  return text.split(/(?<=\.) |\n/)[0].trim();
}

/** The tab order: the access classes a server's tools can declare. */
const ACCESS_ORDER: ToolAccess[] = ['read', 'write', 'destructive', 'undeclared'];

/** i18n label key per access tab. */
const LABEL: Record<
  ToolAccess,
  'mcpAccessRead' | 'mcpAccessWrite' | 'mcpAccessDestructive' | 'mcpAccessUndeclared'
> = {
  read: 'mcpAccessRead',
  write: 'mcpAccessWrite',
  destructive: 'mcpAccessDestructive',
  undeclared: 'mcpAccessUndeclared'
};

function ServerCard({
  index,
  server,
  connecting,
  views,
  onPatch,
  onTools,
  onRemove,
  onConnect,
  onAuthorize
}: {
  index: number;
  server: McpServer;
  connecting: boolean;
  views: McpToolSummary[];
  onPatch: (patch: Partial<McpServer>) => void;
  /** A tool switched on or off: stored at once, no Save needed. */
  onTools: (disabledTools: string[]) => void;
  onRemove: () => void;
  onConnect: () => void;
  onAuthorize: () => void;
}) {
  function toggleTool(name: string) {
    const disabled = server.disabledTools.includes(name)
      ? server.disabledTools.filter((tool) => tool !== name)
      : [...server.disabledTools, name];
    onTools(disabled);
  }

  const t = useT();
  const lang = useLang();
  const [tab, setTab] = useState<ToolAccess | null>(null);

  // The backend view is the source for access and token cost; a missing entry
  // means the tool was not listed by the settings route, so it is undeclared.
  const viewByName = new Map(views.map((view) => [view.name, view] as [string, McpToolSummary]));
  const tools = server.cachedTools.map((tool) => {
    const view = viewByName.get(tool.name);
    return {
      tool,
      access: view?.access ?? 'undeclared',
      tokens: view?.tokens ?? 0
    };
  });
  const present = ACCESS_ORDER.filter((access) => tools.some((entry) => entry.access === access));
  const activeTab = tab && present.includes(tab) ? tab : present[0] ?? null;
  const toolsOf = (access: ToolAccess) => tools.filter((entry) => entry.access === access);
  const enabled = tools.filter((entry) => !server.disabledTools.includes(entry.tool.name)).length;
  const enabledTokens = tools.reduce(
    (sum, entry) => (server.disabledTools.includes(entry.tool.name) ? sum : sum + entry.tokens),
    0
  );
  const activeTools = activeTab ? toolsOf(activeTab) : [];
  const tabEnabledTokens = activeTools.reduce(
    (sum, entry) => (server.disabledTools.includes(entry.tool.name) ? sum : sum + entry.tokens),
    0
  );
  const allTabEnabled =
    activeTools.length > 0 &&
    activeTools.every((entry) => !server.disabledTools.includes(entry.tool.name));
  const someTabEnabled = activeTools.some(
    (entry) => !server.disabledTools.includes(entry.tool.name)
  );

  function toggleTab(access: ToolAccess) {
    const names = toolsOf(access).map((entry) => entry.tool.name);
    const allEnabled = names.every((name) => !server.disabledTools.includes(name));
    if (allEnabled) {
      // Disable every tool in the tab: keep the existing disabled order, then
      // append the tools that were still enabled.
      const missing = names.filter((name) => !server.disabledTools.includes(name));
      onTools([...server.disabledTools, ...missing]);
    } else {
      // Enable every tool in the tab: remove only those names, keeping order.
      onTools(server.disabledTools.filter((name) => !names.includes(name)));
    }
  }
  return (
    <Card variant="inset">
      <div className="flex items-center gap-2">
        <TextInput
          value={server.name}
          onChange={(event) => onPatch({ name: event.target.value })}
          placeholder={t('mcpNamePlaceholder')}
          className="w-40"
          aria-label={t('mcpNameLabel')}
        />
        <Select
          value={server.transport}
          onChange={(event) => onPatch({ transport: event.target.value as McpServer['transport'] })}
          className="w-28"
          aria-label={t('mcpTransportLabel')}
        >
          <option value="stdio">stdio</option>
          <option value="http">http</option>
        </Select>
        {server.transport === 'stdio' ? (
          <>
            <TextInput
              value={server.command ?? ''}
              onChange={(event) => onPatch({ command: event.target.value })}
              placeholder={t('mcpCommandPlaceholder')}
              className="w-40"
              aria-label={t('mcpCommandLabel')}
            />
            <TextInput
              value={server.args.join(' ')}
              onChange={(event) =>
                onPatch({ args: event.target.value.split(/\s+/).filter((part) => part !== '') })
              }
              placeholder={t('mcpArgsPlaceholder')}
              className="w-40"
              aria-label={t('mcpArgsLabel')}
            />
          </>
        ) : (
          <TextInput
            value={server.url ?? ''}
            onChange={(event) => onPatch({ url: event.target.value })}
            placeholder={t('mcpUrlPlaceholder')}
            className="w-64"
            aria-label={t('mcpUrlLabel')}
          />
        )}
        <div className="ml-auto flex items-center gap-2">
          {server.transport === 'http' ? (
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              onClick={onAuthorize}
              disabled={connecting || server.name === ''}
              title={t('mcpAuthorize')}
              aria-label={t('mcpAuthorize')}
            >
              <KeyIcon className="h-4 w-4" />
            </Button>
          ) : null}
          <Button
            variant="soft"
            size="sm"
            onClick={onConnect}
            disabled={connecting || server.name === ''}
          >
            {connecting ? t('mcpConnecting') : t('mcpConnect')}
          </Button>
          <Button variant="ghost" size="sm" iconOnly onClick={onRemove} aria-label={t('mcpRemove')}>
            ×
          </Button>
        </div>
      </div>

      <div className="flex items-center gap-4">
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={!server.disabled}
            onChange={(event) => onPatch({ disabled: !event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          {t('mcpActive')}
        </label>
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={server.visible}
            onChange={(event) => onPatch({ visible: event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          {t('mcpShown')}
        </label>
        {server.cachedTools.length > 0 && (
          <span className="text-xs text-muted">
            {t('mcpToolCount', { count: server.cachedTools.length, s: server.cachedTools.length > 1 ? 's' : '' })}
          </span>
        )}
      </div>

      <div className="flex items-center gap-2">
        <label className="text-sm text-text" htmlFor={`mcp-auth-${index}`}>
          {t('mcpAuthTokenLabel')}
        </label>
        <TextInput
          id={`mcp-auth-${index}`}
          type="password"
          value={server.authToken ?? ''}
          onChange={(event) => onPatch({ authToken: event.target.value || null })}
          placeholder={t('mcpAuthTokenPlaceholder')}
          className="w-64"
          aria-label={t('mcpAuthTokenLabel')}
        />
        <span className="text-xs text-muted">
          {server.transport === 'http' ? t('mcpAuthTokenBearer') : t('mcpAuthTokenEnv')}
        </span>
      </div>

      {server.cachedTools.length > 0 && (
        <ListCard
          folded
          title={t('mcpTools')}
          count={`${enabled}/${server.cachedTools.length} · ${t('mcpTokens', { count: formatTokens(lang, enabledTokens) })}`}
        >
          {activeTab ? (
            <div
              className="border-b border-border px-3 pt-1"
              title={t('mcpAccessDeclared')}
            >
              <Tabs
                label={t('mcpToolsByAccess')}
                orientation="horizontal"
                tabs={present.map((access) => ({
                  id: access,
                  label: t(LABEL[access]),
                  count: toolsOf(access).length
                }))}
                active={activeTab}
                onSelect={setTab}
                trailing={
                  <label className="flex items-center gap-2 text-xs text-muted">
                    <input
                      type="checkbox"
                      checked={allTabEnabled}
                      onChange={() => toggleTab(activeTab)}
                      ref={(el) => {
                        if (el) el.indeterminate = someTabEnabled && !allTabEnabled;
                      }}
                      className="h-4 w-4 accent-[var(--accent)]"
                      aria-label={t('mcpToggleTab')}
                    />
                    <span className="tabular-nums">{t('mcpTokens', { count: formatTokens(lang, tabEnabledTokens) })}</span>
                  </label>
                }
              />
            </div>
          ) : null}
          {activeTools.map((entry, toolIndex) => {
            const disabled = server.disabledTools.includes(entry.tool.name);
            return (
              <details key={entry.tool.name} className={toolIndex > 0 ? 'border-t border-border' : undefined}>
                <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-1.5 hover:bg-accent/5">
                  <input
                    type="checkbox"
                    checked={!disabled}
                    onChange={() => toggleTool(entry.tool.name)}
                    onClick={(event) => event.stopPropagation()}
                    className="h-4 w-4 accent-[var(--accent)]"
                    aria-label={entry.tool.name}
                  />
                  <span
                    className={
                      disabled
                        ? 'shrink-0 font-mono text-xs text-faint line-through'
                        : 'shrink-0 font-mono text-xs text-text'
                    }
                  >
                    {entry.tool.name}
                  </span>
                  {entry.tool.description ? (
                    <span className="min-w-0 flex-1 truncate text-xs text-faint">
                      {firstSentence(entry.tool.description)}
                    </span>
                  ) : null}
                  <span className="shrink-0 text-xs tabular-nums text-faint">~{formatTokens(lang, entry.tokens)}</span>
                </summary>
                {entry.tool.description ? (
                  <p className="whitespace-pre-wrap px-3 pb-2 pl-9 text-xs text-muted">{entry.tool.description}</p>
                ) : null}
              </details>
            );
          })}
        </ListCard>
      )}
    </Card>
  );
}

export function McpSection({
  value,
  onChange,
  toolViews,
  save,
  onConnected,
  onToolsChange
}: {
  value: McpServer[];
  onChange: (value: McpServer[]) => void;
  /** Per server, the settings route's tool access and token estimates. */
  toolViews: Record<string, McpToolSummary[]>;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
  /** After a connect: the server stored the tools, the page reads them back. */
  onConnected: () => Promise<void>;
  /** A server's tools switched on or off, stored at once: a toggle is a decision, not a draft. */
  onToolsChange: (name: string, disabledTools: string[]) => Promise<void>;
}) {
  const t = useT();
  const [connecting, setConnecting] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  function patch(index: number, p: Partial<McpServer>) {
    onChange(value.map((server, i) => (i === index ? { ...server, ...p } : server)));
  }

  /** The server's form, saved first if edited: the server reads it from the store. */
  async function saveFirst() {
    if (save?.dirty) await save.onSave();
  }

  async function connect(index: number) {
    const name = value[index].name;
    if (name === '') return;
    setConnecting(index);
    setError(null);
    try {
      await saveFirst();
      await connectMcp(name);
      await onConnected();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(null);
    }
  }

  async function authorize(index: number) {
    setError(null);
    // Opened during the click: a tab opened after the awaits would be blocked.
    const tab = window.open('', '_blank');
    if (tab) tab.opener = null;
    try {
      await saveFirst();
      const { url } = await authorizeMcp(value[index].name);
      // The consent happens in another tab; when the human comes back, the
      // tokens are stored and a connect loads the tools into this form.
      window.addEventListener('focus', () => void connect(index), { once: true });
      if (tab) tab.location.href = url;
      else window.open(url, '_blank', 'noopener');
    } catch (e) {
      tab?.close();
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <Section
      title={t('mcpTitle')}
      description={t('mcpHint')}
      actions={
        <Button variant="primary" onClick={() => onChange([...value, emptyServer()])}>
          {t('newServer')}
        </Button>
      }
      save={save}
    >
      {error ? <p className="text-xs text-err">{error}</p> : null}
      {value.length === 0 ? (
        <p className="text-sm text-muted">{t('noMcpServers')}</p>
      ) : (
        value.map((server, index) => (
          <ServerCard
            key={index}
            index={index}
            server={server}
            connecting={connecting === index}
            views={toolViews[server.name] ?? []}
            onPatch={(p) => patch(index, p)}
            onTools={(disabledTools) => {
              patch(index, { disabledTools });
              onToolsChange(server.name, disabledTools).catch((e) =>
                setError(e instanceof Error ? e.message : String(e))
              );
            }}
            onRemove={() => onChange(value.filter((_, i) => i !== index))}
            onConnect={() => connect(index)}
            onAuthorize={() => authorize(index)}
          />
        ))
      )}
    </Section>
  );
}
