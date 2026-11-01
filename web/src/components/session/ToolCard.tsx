import { useEffect, useState } from 'react';
import type { ToolCallView } from '../../lib/fold';
import { useDisplayPrefs } from '../../lib/display';
import { formatDuration } from '../../lib/format';
import { previewKindOfPath, relativeToWorkdir } from '../../lib/preview';
import { usePreviewOpener, usePreviewWorkdir } from '../preview/PreviewContext';
import { EditIcon, EyeIcon, FileIcon, SearchIcon, SpinnerIcon, TerminalIcon } from '../Icons';
import { toolAnchorId } from '../../lib/vision';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

/** A silent command still shows it is alive: the time since the card appeared. */
// ponytail: counts from mount, so a reload restarts it; the events carry no timestamp.
function RunningSince() {
  const [start] = useState(() => Date.now());
  const [now, setNow] = useState(start);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  return (
    <div className="text-muted">
      En cours, pas encore de sortie · {formatDuration(now - start)}
    </div>
  );
}

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
        : tool.name === 'web_search'
          ? String(tool.args.query ?? '')
          : String(tool.args.path ?? '');
  const lineCount = tool.output ? tool.output.split('\n').length : 0;
  const opener = usePreviewOpener();
  const workdir = usePreviewWorkdir();
  const previewPath = String(tool.args.path ?? '');
  const previewKind =
    tool.name === 'write_file' || tool.name === 'edit_file' ? previewKindOfPath(previewPath) : undefined;
  const previewRelative =
    previewKind && workdir ? relativeToWorkdir(previewPath, workdir) : undefined;

  const icon =
    tool.name === 'run_command' ? (
      <TerminalIcon className="h-[15px] w-[15px] text-muted" />
    ) : tool.name === 'edit_file' ? (
      <EditIcon className="h-[15px] w-[15px] text-muted" />
    ) : tool.name === 'web_search' ? (
      <SearchIcon className="h-[15px] w-[15px] text-muted" />
    ) : (
      <FileIcon className="h-[15px] w-[15px] text-muted" />
    );

  return (
    <Card id={toolAnchorId(tool.id)} className="overflow-hidden">
      <div className="flex min-h-10 items-center gap-2.5 px-3 text-left text-text">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
          className="flex min-w-0 grow items-center gap-2.5"
        >
          {icon}
          <span className="shrink-0 font-mono text-xs font-medium">{tool.name}</span>
          <span title={arg} className="min-w-0 truncate font-mono text-xs text-muted">
            {arg}
          </span>
          <span className="grow" />
        </button>
        {tool.status === 'success' && previewKind && previewRelative && opener && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            onClick={(event) => {
              event.stopPropagation();
              opener({ path: previewRelative, kind: previewKind });
            }}
            className="shrink-0"
            title="Aperçu"
            aria-label="Aperçu"
          >
            <EyeIcon className="h-4 w-4" />
          </Button>
        )}
        {tool.name === 'edit_file' && (
          <>
            <span className="font-mono text-xs text-ok">+{tool.addedLines ?? 0}</span>
            <span className="font-mono text-xs text-muted">−{tool.removedLines ?? 0}</span>
          </>
        )}
        {tool.name === 'read_file' && lineCount > 0 && (
          <span className="shrink-0 text-xs whitespace-nowrap text-muted">{lineCount} lignes</span>
        )}
        {tool.blockedBy && (
          <Badge tone="err" className="shrink-0" title={`Refusé par la règle ${tool.blockedBy.scope === 'global' ? 'globale' : 'du projet'} « ${tool.blockedBy.pattern} »`}>
            vérifié · règle
          </Badge>
        )}
        {tool.name === 'run_command' && tool.status !== 'running' && tool.exitCode != null && (
          <Badge tone={tool.exitCode === 0 ? 'ok' : 'err'} className="shrink-0">
            vérifié · exit {tool.exitCode}
          </Badge>
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
      </div>

      {open && (
        <div className="border-t border-border">
          {tool.name === 'read_file' && (
            tool.image ? (
              <img src={tool.image.dataUrl} alt={tool.output} className="max-h-80 p-2.5" />
            ) : (
              <pre className="overflow-x-auto bg-code p-2.5 font-mono text-xs text-muted whitespace-pre">
                {tool.output || 'Aucune sortie'}
              </pre>
            )
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
                  {!tool.stdout && !tool.stderr && <RunningSince />}
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
    </Card>
  );
}
