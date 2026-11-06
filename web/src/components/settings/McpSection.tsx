import { useState } from 'react';
import { Section, type SectionSave } from '../ui/Section';
import { TextInput, Select } from '../ui/Field';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { authorizeMcp, connectMcp, type McpServer } from '../../lib/api';
import { KeyIcon } from '../Icons';
import { useT } from '../../i18n';

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

function ServerCard({
  index,
  server,
  connecting,
  saved,
  onPatch,
  onRemove,
  onConnect,
  onAuthorize
}: {
  index: number;
  server: McpServer;
  connecting: boolean;
  saved: boolean;
  onPatch: (patch: Partial<McpServer>) => void;
  onRemove: () => void;
  onConnect: () => void;
  onAuthorize: () => void;
}) {
  function toggleTool(name: string) {
    const disabled = server.disabledTools.includes(name)
      ? server.disabledTools.filter((tool) => tool !== name)
      : [...server.disabledTools, name];
    onPatch({ disabledTools: disabled });
  }

  const t = useT();
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
              disabled={connecting || server.name === '' || !saved}
              title={!saved ? t('mcpSaveFirst') : t('mcpAuthorize')}
              aria-label={t('mcpAuthorize')}
            >
              <KeyIcon className="h-4 w-4" />
            </Button>
          ) : null}
          <Button
            variant="soft"
            size="sm"
            onClick={onConnect}
            disabled={connecting || server.name === '' || !saved}
            title={!saved ? t('mcpSaveFirst') : undefined}
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
        <div className="flex flex-col gap-1">
          {server.cachedTools.map((tool) => (
            <label key={tool.name} className="flex items-center gap-2 text-sm text-text">
              <input
                type="checkbox"
                checked={!server.disabledTools.includes(tool.name)}
                onChange={() => toggleTool(tool.name)}
                className="h-4 w-4 accent-[var(--accent)]"
              />
              {server.name}_{tool.name}
              {tool.description ? (
                <span className="text-xs text-muted">— {tool.description}</span>
              ) : null}
            </label>
          ))}
        </div>
      )}
    </Card>
  );
}

export function McpSection({
  value,
  onChange,
  savedNames,
  save
}: {
  value: McpServer[];
  onChange: (value: McpServer[]) => void;
  savedNames: Set<string>;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}) {
  const t = useT();
  const [connecting, setConnecting] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  function patch(index: number, p: Partial<McpServer>) {
    onChange(value.map((server, i) => (i === index ? { ...server, ...p } : server)));
  }

  async function connect(index: number) {
    const name = value[index].name;
    if (name === '') return;
    setConnecting(index);
    setError(null);
    try {
      const { server } = await connectMcp(name);
      patch(index, { cachedTools: server.cachedTools });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(null);
    }
  }

  async function authorize(index: number) {
    setError(null);
    try {
      const { url } = await authorizeMcp(value[index].name);
      // The consent happens in another tab; when the human comes back, the
      // tokens are stored and a connect loads the tools into this form.
      window.addEventListener('focus', () => void connect(index), { once: true });
      window.open(url, '_blank', 'noopener');
    } catch (e) {
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
            saved={savedNames.has(server.name)}
            onPatch={(p) => patch(index, p)}
            onRemove={() => onChange(value.filter((_, i) => i !== index))}
            onConnect={() => connect(index)}
            onAuthorize={() => authorize(index)}
          />
        ))
      )}
    </Section>
  );
}
