// Settings' MCP tab: accordion server list, connect/save state, and per-card actions.
import { useEffect, useRef, useState } from 'react';
import { authorizeMcp, connectMcp, type McpServer } from '../../lib/api';
import type { McpToolSummary } from '../../lib/api/settings';
import type { SectionSave } from '../ui/Section';
import { InfoBar } from '../ui/InfoBar';
import { McpServerCard } from './McpServerCard';
import { useT } from '../../i18n';

export function emptyServer(): McpServer {
  return {
    name: '',
    transport: 'stdio',
    command: '',
    args: [],
    env: {},
    url: '',
    headers: {},
    disabled: true,
    visible: true,
    disabledTools: [],
    cachedTools: [],
    timeout: 0
  };
}

export function McpSection({
  value,
  onChange,
  toolViews,
  save,
  onConnected
}: {
  value: McpServer[];
  onChange: (value: McpServer[]) => void;
  /** Per server, the settings route's tool access and token estimates. */
  toolViews: Record<string, McpToolSummary[]>;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
  /** After a connect: the server stored the tools, the page reads them back. */
  onConnected: () => Promise<void>;
}) {
  const t = useT();
  const [connecting, setConnecting] = useState<number | null>(null);
  const [error, setError] = useState<{ index: number; message: string } | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const prevLength = useRef(value.length);

  useEffect(() => {
    if (value.length > prevLength.current) {
      setOpen(value.length - 1);
    } else if (value.length < prevLength.current) {
      setOpen(null);
    }
    prevLength.current = value.length;
  }, [value.length]);

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
      setError({ index, message: e instanceof Error ? e.message : String(e) });
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
      window.addEventListener('focus', () => void connect(index), {
        once: true
      });
      if (tab) tab.location.href = url;
      else window.open(url, '_blank', 'noopener');
    } catch (e) {
      tab?.close();
      setError({ index, message: e instanceof Error ? e.message : String(e) });
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <InfoBar>{t('mcpHint')}</InfoBar>
      {/* An open card shows its own save state; a switch flipped on a closed row reports here. */}
      {open === null && save?.auto?.error ? <p className="m-0 text-xs text-err">{save.auto.error}</p> : null}
      {value.length === 0 ? (
        <p className="rounded-lg border border-dashed border-border px-4 py-3 text-sm text-muted">
          {t('noMcpServers')}
        </p>
      ) : (
        value.map((server, index) => (
          <McpServerCard
            key={index}
            index={index}
            server={server}
            open={open === index}
            onToggle={() => setOpen(open === index ? null : index)}
            connecting={connecting === index}
            views={toolViews[server.name] ?? []}
            saveState={
              open === index
                ? {
                    saving: save?.auto?.saving ?? false,
                    error: save?.auto?.error ?? null
                  }
                : { saving: false, error: null }
            }
            actionError={error?.index === index ? error.message : null}
            onPatch={(p) => patch(index, p)}
            onRemove={() => onChange(value.filter((_, i) => i !== index))}
            onConnect={() => connect(index)}
            onAuthorize={() => authorize(index)}
          />
        ))
      )}
    </div>
  );
}
