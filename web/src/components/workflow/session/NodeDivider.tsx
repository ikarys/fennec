import { useEffect, useRef, useState } from 'react';
import type { ThreadItem } from '../../../lib/fold';
import { formatClock, formatDuration } from '../../../lib/format';
import { portTone } from '../../../lib/workflowRun';
import { CheckIcon, CloseIcon, SpinnerIcon } from '../../Icons';

type WorkflowNodeItem = Extract<ThreadItem, { kind: 'workflow-node' }>;

interface PortBadge {
  className: string;
  title: string;
  icon?: 'check' | 'close';
  label?: string;
}

function portBadge(port: string): PortBadge {
  const tone = portTone(port);
  if (tone === 'positive') {
    return { className: 'text-ok border-ok bg-ok/10', title: port, icon: 'check' };
  }
  if (tone === 'negative') {
    return { className: 'text-err border-err bg-err/10', title: port, icon: 'close' };
  }
  return { className: 'text-accent-strong border-accent bg-accent-soft', title: port, label: port };
}

export function NodeDivider({ item, running }: { item: WorkflowNodeItem; running: boolean }) {
  const finished = item.port !== undefined;
  const badge = finished ? portBadge(item.port!) : null;
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
            <span
              className={`inline-flex items-center rounded-full border px-1.5 text-[0.625rem] ${badge.className}`}
              title={badge.title}
              aria-label={badge.title}
            >
              {badge.icon === 'check' && <CheckIcon className="h-3 w-3" />}
              {badge.icon === 'close' && <CloseIcon className="h-3 w-3" />}
              {badge.label}
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
        <details className="mx-auto">
          <summary className="cursor-pointer text-center text-[0.6875rem] text-muted hover:text-text">détails</summary>
          <div className="mt-1.5 flex flex-col gap-2 rounded-md border border-border bg-raised px-3 py-2 text-left text-xs">
            {item.prompt && (
              <div>
                <div className="mb-1 text-[0.625rem] uppercase tracking-wide text-muted">Prompt</div>
                <div className="whitespace-pre-wrap text-text">{item.prompt}</div>
              </div>
            )}
            {item.output && (
              <div>
                <div className="mb-1 text-[0.625rem] uppercase tracking-wide text-muted">Sortie</div>
                {(item.nodeKind === 'shell' || item.nodeKind === 'criteria') ? (
                  <pre className="overflow-x-auto rounded bg-code p-2 font-mono text-[0.6875rem] text-muted">{item.output}</pre>
                ) : (
                  <div className="whitespace-pre-wrap text-text">{item.output}</div>
                )}
                {item.nodeKind === 'shell' && item.exitCode !== undefined && (
                  <div className="mt-1 text-[0.6875rem] text-muted">Code de sortie : {item.exitCode}</div>
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
