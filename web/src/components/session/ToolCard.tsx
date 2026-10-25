import { useEffect, useState } from 'react';
import type { ToolCallView } from '../../lib/fold';
import { useDisplayPrefs } from '../../lib/display';
import { formatDuration } from '../../lib/format';
import { EditIcon, FileIcon, SpinnerIcon, TerminalIcon } from '../Icons';

export function ToolCard({ tool }: { tool: ToolCallView }) {
  const display = useDisplayPrefs();
  const [open, setOpen] = useState(display.toolResultsOpen);

  useEffect(() => {
    if (tool.name === 'run_command' && tool.status === 'running') {
      setOpen(true);
    }
  }, [tool.name, tool.status]);

  const arg =
    tool.name === 'ask_user'
      ? String(tool.question ?? tool.args.question ?? '')
      : tool.name === 'run_command'
        ? String(tool.args.command ?? '')
        : String(tool.args.path ?? '');
  const lineCount = tool.output ? tool.output.split('\n').length : 0;

  const icon =
    tool.name === 'run_command' ? (
      <TerminalIcon className="h-[15px] w-[15px] text-muted" />
    ) : tool.name === 'edit_file' ? (
      <EditIcon className="h-[15px] w-[15px] text-muted" />
    ) : (
      <FileIcon className="h-[15px] w-[15px] text-muted" />
    );

  return (
    <div className="overflow-hidden rounded-[10px] border border-border bg-panel">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex min-h-10 w-full items-center gap-2.5 px-3 text-left text-text"
      >
        {icon}
        <span className="shrink-0 font-mono text-[12.5px] font-medium">{tool.name}</span>
        <span title={arg} className="min-w-0 truncate font-mono text-[12.5px] text-muted">
          {arg}
        </span>
        <span className="grow" />
        {tool.name === 'edit_file' && (
          <>
            <span className="font-mono text-xs text-ok">+{tool.addedLines ?? 0}</span>
            <span className="font-mono text-xs text-muted">−{tool.removedLines ?? 0}</span>
          </>
        )}
        {tool.name === 'read_file' && lineCount > 0 && (
          <span className="shrink-0 text-xs whitespace-nowrap text-muted">{lineCount} lignes</span>
        )}
        {tool.name === 'run_command' && tool.status !== 'running' && tool.exitCode != null && (
          <span
            className={`shrink-0 whitespace-nowrap rounded px-1.5 py-px text-[11px] ${
              tool.exitCode === 0 ? 'bg-ok-soft text-ok' : 'bg-raised text-err'
            }`}
          >
            vérifié · exit {tool.exitCode}
          </span>
        )}
        {tool.durationMs != null && (
          <span className="shrink-0 text-xs whitespace-nowrap text-muted">{formatDuration(tool.durationMs)}</span>
        )}
        {tool.status === 'running' && tool.name !== 'ask_user' && <SpinnerIcon className="h-4 w-4 text-accent" />}
        {tool.status === 'running' && tool.name === 'ask_user' && (
          <span className="shrink-0 text-xs whitespace-nowrap text-muted">en attente</span>
        )}
        {tool.status === 'success' && <span className="text-xs font-semibold text-ok">✓</span>}
        {tool.status === 'error' && <span className="text-xs font-semibold text-err">✗</span>}
      </button>

      {open && (
        <div className="border-t border-border">
          {tool.name === 'read_file' && (
            <pre className="overflow-x-auto bg-code p-2.5 font-mono text-xs text-muted whitespace-pre">
              {tool.output || 'Aucune sortie'}
            </pre>
          )}

          {tool.name === 'edit_file' && (
            <div className="overflow-x-auto bg-code font-mono text-xs">
              {/* w-max: line backgrounds span the longest line when scrolled. */}
              <div className="w-max min-w-full">
                {(tool.diff?.removed.length ?? 0) === 0 && (tool.diff?.added.length ?? 0) === 0 && (
                  <div className="px-3.5 py-1 text-muted">Aucun changement</div>
                )}
                {tool.diff?.removed.map((line, index) => (
                  <div key={`removed-${index}`} className="bg-del-bg px-3.5 py-px whitespace-pre text-text">
                    <span className="text-err">- </span>
                    {line}
                  </div>
                ))}
                {tool.diff?.added.map((line, index) => (
                  <div key={`added-${index}`} className="bg-add-bg px-3.5 py-px whitespace-pre text-text">
                    <span className="text-ok">+ </span>
                    {line}
                  </div>
                ))}
              </div>
            </div>
          )}

          {tool.name === 'write_file' && (
            <pre className="overflow-x-auto bg-code p-2.5 font-mono text-xs text-muted whitespace-pre">
              {(tool.contentPreview ?? []).join('\n') || 'Aucun contenu'}
            </pre>
          )}

          {tool.name === 'run_command' && (
            <div className="overflow-x-auto bg-code p-2.5 font-mono text-xs whitespace-pre">
              {tool.status === 'running' ? (
                <>
                  {tool.stdout && <div>{tool.stdout}</div>}
                  {tool.stderr && <div className="text-err">{tool.stderr}</div>}
                </>
              ) : (
                <div>{tool.output || tool.error || 'Aucune sortie'}</div>
              )}
            </div>
          )}

          {tool.name === 'ask_user' && (
            <pre className="overflow-x-auto bg-code p-2.5 font-mono text-xs text-muted whitespace-pre">
              {tool.output || tool.error || 'En attente de réponse'}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
