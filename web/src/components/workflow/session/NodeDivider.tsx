import type { ThreadItem } from '../../../lib/fold';

type WorkflowNodeItem = Extract<ThreadItem, { kind: 'workflow-node' }>;

function portBadge(port: string): { label: string; className: string } {
  if (port === 'success' || port === 'done' || port === 'true') {
    return { label: 'ok', className: 'text-ok border-ok bg-ok/10' };
  }
  if (port === 'failure' || port === 'false' || port === 'exhausted') {
    return { label: 'err', className: 'text-err border-err bg-err/10' };
  }
  return { label: port, className: 'text-accent-strong border-accent bg-accent-soft' };
}

export function NodeDivider({ item }: { item: WorkflowNodeItem }) {
  const finished = item.port !== undefined;
  const badge = finished ? portBadge(item.port!) : null;
  const passLabel = item.pass > 1 ? ` · passage ${item.pass}` : '';

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-[11px] text-muted">
        <span className="h-px grow bg-border" />
        <span className="flex items-center gap-1.5">
          <span className="rounded border border-border bg-raised px-1.5 text-[10px] uppercase tracking-wide">{item.nodeKind}</span>
          <span className="text-text">{item.label}{passLabel}</span>
          {badge && (
            <span className={`rounded-full border px-1.5 text-[10px] ${badge.className}`}>{badge.label}</span>
          )}
        </span>
        <span className="h-px grow bg-border" />
      </div>
      {(item.prompt || item.output || item.childSessionId) && (
        <details className="mx-auto">
          <summary className="cursor-pointer text-center text-[11px] text-muted hover:text-text">détails</summary>
          <div className="mt-1.5 flex flex-col gap-2 rounded-md border border-border bg-raised px-3 py-2 text-left text-xs">
            {item.prompt && (
              <div>
                <div className="mb-1 text-[10px] uppercase tracking-wide text-muted">Prompt</div>
                <div className="whitespace-pre-wrap text-text">{item.prompt}</div>
              </div>
            )}
            {item.output && (
              <div>
                <div className="mb-1 text-[10px] uppercase tracking-wide text-muted">Sortie</div>
                {item.nodeKind === 'shell' ? (
                  <pre className="overflow-x-auto rounded bg-code p-2 font-mono text-[11px] text-muted">{item.output}</pre>
                ) : (
                  <div className="whitespace-pre-wrap text-text">{item.output}</div>
                )}
                {item.exitCode !== undefined && (
                  <div className="mt-1 text-[11px] text-muted">Code de sortie : {item.exitCode}</div>
                )}
              </div>
            )}
            {item.childSessionId && (
              <a href={`#/s/${item.childSessionId}`} className="text-accent-strong underline">
                Ouvrir la session
              </a>
            )}
          </div>
        </details>
      )}
    </div>
  );
}
