import { useEffect, useRef, useState } from 'react';
import type { McpSessionView, McpServerView } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';

interface McpPickerProps {
  view: McpSessionView | null;
  onToggle: (name: string, active: boolean) => void;
  disabled?: boolean;
}

/**
 * The session's MCP servers in the composer: which are active for the session
 * and the token cost of their tools. Toggling a server records the session's
 * disabled set; the dropdown only lists the servers the human showed.
 */
export function McpPicker({ view, onToggle, disabled }: McpPickerProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false);
    }

    function handleMouseDown(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousedown', handleMouseDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('mousedown', handleMouseDown);
    };
  }, [open]);

  const visible = view ? view.servers.filter((server) => server.visible) : [];
  const activeCount = visible.filter((server) => server.active).length;
  const anyActive = activeCount > 0;
  const label =
    view && visible.length > 0
      ? `${activeCount} MCP${view.totalTokens > 0 ? ` (${view.totalTokens})` : ''}`
      : 'MCP';

  function row(server: McpServerView) {
    return (
      <label key={server.name} className="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-sm text-text hover:bg-bg">
        <span
          className="h-2 w-2 shrink-0 rounded-full"
          style={{ backgroundColor: server.active ? 'var(--ok)' : 'var(--border)' }}
          aria-hidden
        />
        <input
          type="checkbox"
          checked={server.active}
          onChange={() => onToggle(server.name, !server.active)}
          className="h-4 w-4 accent-[var(--accent)]"
        />
        <span className="min-w-0 grow truncate">{server.name}</span>
        <span className="shrink-0 text-xs text-muted">
          {server.toolCount} outil{server.toolCount > 1 ? 's' : ''}
        </span>
        {server.active && server.tokens > 0 && (
          <span className="shrink-0 text-xs text-muted">{server.tokens}</span>
        )}
      </label>
    );
  }

  return (
    <div ref={containerRef} className="relative">
      <Button
        variant="ghost"
        size="sm"
        disabled={disabled}
        title="Serveurs MCP de la session"
        onClick={() => setOpen((value) => !value)}
      >
        <span className="flex items-center gap-1.5">
          <span
            className="h-2 w-2 rounded-full"
            style={{ backgroundColor: anyActive ? 'var(--ok)' : 'var(--border)' }}
            aria-hidden
          />
          {label}
        </span>
      </Button>
      {open && (
        <Card className="absolute bottom-full right-0 z-10 mb-1 w-72 max-h-80 overflow-y-auto p-2">
          <SectionLabel>Serveurs MCP</SectionLabel>
          {visible.length === 0 ? (
            <p className="m-0 px-1.5 text-xs text-muted">
              Aucun serveur affiché. Ajoutez-en dans les réglages (MCP).
            </p>
          ) : (
            <div className="flex flex-col gap-0.5">{visible.map(row)}</div>
          )}
        </Card>
      )}
    </div>
  );
}
