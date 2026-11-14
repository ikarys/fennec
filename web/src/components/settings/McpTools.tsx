// The MCP server's tool access tabs and per-tool enable list.
import { useState } from 'react';
import { Tabs } from '../ui/Tabs';
import { useLang, useT } from '../../i18n';
import { formatTokens } from '../../lib/format';
import type { McpServer } from '../../lib/api';
import type { McpToolSummary, ToolAccess } from '../../lib/api/settings';

/** The description's first sentence, for the one-line summary of a tool row. */
function firstSentence(text: string): string {
  return text.split(/(?<=\.) |\n/)[0].trim();
}

/** The tab order: the access classes a server's tools can declare. */
const ACCESS_ORDER: ToolAccess[] = ['read', 'write', 'destructive', 'undeclared'];

/** i18n label key per access tab. */
const LABEL: Record<ToolAccess, 'mcpAccessRead' | 'mcpAccessWrite' | 'mcpAccessDestructive' | 'mcpAccessUndeclared'> = {
  read: 'mcpAccessRead',
  write: 'mcpAccessWrite',
  destructive: 'mcpAccessDestructive',
  undeclared: 'mcpAccessUndeclared'
};

export function enabledToolStats(
  server: McpServer,
  views: McpToolSummary[]
): { enabled: number; total: number; tokens: number } {
  const viewByName = new Map(views.map((view) => [view.name, view] as [string, McpToolSummary]));
  let enabled = 0;
  let tokens = 0;
  for (const tool of server.cachedTools) {
    if (!server.disabledTools.includes(tool.name)) {
      enabled += 1;
      tokens += viewByName.get(tool.name)?.tokens ?? 0;
    }
  }
  return { enabled, total: server.cachedTools.length, tokens };
}

export function McpTools({
  server,
  views,
  onPatch
}: {
  server: McpServer;
  views: McpToolSummary[];
  onPatch: (patch: Partial<McpServer>) => void;
}) {
  const t = useT();
  const lang = useLang();
  const [tab, setTab] = useState<ToolAccess | null>(null);

  function toggleTool(name: string) {
    const disabled = server.disabledTools.includes(name)
      ? server.disabledTools.filter((tool) => tool !== name)
      : [...server.disabledTools, name];
    onPatch({ disabledTools: disabled });
  }

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
  const activeTab = tab && present.includes(tab) ? tab : (present[0] ?? null);
  const toolsOf = (access: ToolAccess) => tools.filter((entry) => entry.access === access);
  const activeTools = activeTab ? toolsOf(activeTab) : [];
  const tabEnabledTokens = activeTools.reduce(
    (sum, entry) => (server.disabledTools.includes(entry.tool.name) ? sum : sum + entry.tokens),
    0
  );
  const allTabEnabled =
    activeTools.length > 0 && activeTools.every((entry) => !server.disabledTools.includes(entry.tool.name));
  const someTabEnabled = activeTools.some((entry) => !server.disabledTools.includes(entry.tool.name));

  function toggleTab(access: ToolAccess) {
    const names = toolsOf(access).map((entry) => entry.tool.name);
    const allEnabled = names.every((name) => !server.disabledTools.includes(name));
    if (allEnabled) {
      const missing = names.filter((name) => !server.disabledTools.includes(name));
      onPatch({ disabledTools: [...server.disabledTools, ...missing] });
    } else {
      onPatch({
        disabledTools: server.disabledTools.filter((name) => !names.includes(name))
      });
    }
  }

  return (
    <div className="rounded-md border border-border">
      {activeTab ? (
        <div className="border-b border-border px-3 pt-1">
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
                  className="h-4 w-4"
                  aria-label={t('mcpToggleTab')}
                />
                <span className="tabular-nums">
                  {t('mcpTokens', {
                    count: formatTokens(lang, tabEnabledTokens)
                  })}
                </span>
              </label>
            }
          />
          <p className="m-0 mb-2 px-3 text-xs text-faint">{t('mcpAccessDeclared')}</p>
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
                className="h-4 w-4"
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
    </div>
  );
}
