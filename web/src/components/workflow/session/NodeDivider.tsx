import { useEffect, useRef, useState } from 'react';
import type { ThreadItem } from '../../../lib/fold';
import { formatClock, formatDuration } from '../../../lib/format';
import { portTone } from '../../../lib/workflowRun';
import { CheckIcon, CloseIcon, SpinnerIcon } from '../../Icons';
import { Badge } from '../../ui/Badge';
import { SectionLabel } from '../../ui/SectionLabel';

type WorkflowNodeItem = Extract<ThreadItem, { kind: 'workflow-node' }>;

function portBadgeTone(port: string): 'ok' | 'err' | 'accent' {
  const tone = portTone(port);
  if (tone === 'positive') return 'ok';
  if (tone === 'negative') return 'err';
  return 'accent';
}

export function NodeDivider({ item, running }: { item: WorkflowNodeItem; running: boolean }) {
  const finished = item.port !== undefined;
  const badge = finished ? { port: item.port!, tone: portBadgeTone(item.port!) } : null;
  const passLabel = item.pass > 1 ? ` · passage ${item.pass}` : '';
  // Measured in the browser from the first render while the node is running.
  const startRef = useRef<number | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);

  useEffect(() => {
    if (!running || finished) return;
    if (startRef.current === null) {
      startRef.current = Date.now();
    }
    setElapsedMs(Date.now() - startRef.current);
    const interval = window.setInterval(() => {
      setElapsedMs(Date.now() - (startRef.current ?? Date.now()));
    }, 1000);
    return () => window.clearInterval(interval);
  }, [running, finished]);

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-[0.6875rem] text-muted">
        <span className="h-px grow bg-border" />
        <span className="flex items-center gap-1.5">
          <span className="rounded border border-border bg-raised px-1.5 text-[0.625rem] uppercase tracking-wide">{item.nodeKind}</span>
          <span className="text-text">{item.label}{passLabel}</span>
          {badge && (
            <span className="shrink-0" aria-label={badge.port}>
              <Badge tone={badge.tone} title={badge.port}>
                {badge.tone === 'ok' && <CheckIcon className="h-3 w-3" />}
                {badge.tone === 'err' && <CloseIcon className="h-3 w-3" />}
                {badge.tone === 'accent' && badge.port}
              </Badge>
            </span>
          )}
          {finished && item.durationMs != null && (
            <span className="text-muted">{formatDuration(item.durationMs)}</span>
          )}
          {!finished && running && (
            <span className="flex items-center gap-1 text-accent-strong">
              <SpinnerIcon className="h-3 w-3" />
              en cours…
              <span className="tabular-nums">{formatClock(elapsedMs)}</span>
            </span>
          )}
          {!finished && !running && <span className="text-muted">interrompu</span>}
        </span>
        <span className="h-px grow bg-border" />
      </div>
      {(item.prompt || item.output || item.childSessionId) && (
        <details className="w-full min-w-0">
          <summary className="cursor-pointer text-center text-[0.6875rem] text-muted hover:text-text">détails</summary>
          <div className="flex flex-col gap-2 rounded-md border border-border bg-raised px-3 py-2 text-left text-xs">
            {item.prompt && (
              <div className="flex flex-col gap-1.5">
                <SectionLabel>Prompt</SectionLabel>
                <div className="whitespace-pre-wrap text-text">{item.prompt}</div>
              </div>
            )}
            {item.output && (
              <div className="flex flex-col gap-1.5">
                <SectionLabel>Sortie</SectionLabel>
                {(item.nodeKind === 'shell' || item.nodeKind === 'criteria') ? (
                  <pre className="whitespace-pre-wrap break-words rounded bg-code p-2 font-mono text-[0.6875rem] text-muted">{item.output}</pre>
                ) : (
                  <div className="whitespace-pre-wrap text-text">{item.output}</div>
                )}
                {item.nodeKind === 'shell' && item.exitCode !== undefined && (
                  <div className="text-[0.6875rem] text-muted">Code de sortie : {item.exitCode}</div>
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
