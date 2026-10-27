/**
 * The workflow run as a collapsible left rail: vertical, so every step stays visible.
 */
import { useEffect, useMemo, useRef, useState } from 'react';
import { portTone, upcomingSteps, type NodeRun, type RunView } from '../../../lib/workflowRun';
import { CheckIcon, ChevronsLeftIcon, ChevronsRightIcon, CloseIcon, StopIcon } from '../../Icons';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';

const COLLAPSED_KEY = 'fennec.runRail.collapsed';

function loadCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === '1';
  } catch {
    return false;
  }
}

function saveCollapsed(collapsed: boolean): void {
  try {
    localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '0');
  } catch {
    // A blocked storage only loses the rail state on reload.
  }
}

function PortBadge({ port }: { port: string }) {
  const tone = portTone(port);
  if (tone === 'positive') {
    return (
      <span className="text-ok" title={port} aria-label={port}>
        <CheckIcon className="h-3 w-3" />
      </span>
    );
  }
  if (tone === 'negative') {
    return (
      <span className="text-err" title={port} aria-label={port}>
        <CloseIcon className="h-3 w-3" />
      </span>
    );
  }
  return <span className="text-[0.625rem] text-muted" title={port} aria-label={port}>{port}</span>;
}

function outcomeLabel(outcome: string): string {
  switch (outcome) {
    case 'done':
      return 'terminé';
    case 'blocked':
      return 'bloqué';
    case 'stopped':
      return 'arrêté';
    case 'error':
      return 'en erreur';
    default:
      return outcome;
  }
}

function outcomeTone(outcome: string): 'ok' | 'err' | 'neutral' {
  switch (outcome) {
    case 'done':
      return 'ok';
    case 'blocked':
    case 'stopped':
    case 'error':
      return 'err';
    default:
      return 'neutral';
  }
}

function dotClass(nodeRun: NodeRun): string {
  if (nodeRun.state === 'running') {
    return 'animate-pulse bg-accent';
  }
  if (nodeRun.state === 'interrupted') {
    // Solid and faint: dashed is for the steps still to come.
    return 'bg-border-strong';
  }
  if (nodeRun.state === 'ended') {
    return nodeRun.port === 'done' ? 'bg-ok' : 'bg-err';
  }
  if (nodeRun.state === 'done') {
    const tone = portTone(nodeRun.port ?? '');
    if (tone === 'positive') {
      return 'bg-ok';
    }
    if (tone === 'negative') {
      return 'bg-err';
    }
    return 'bg-muted';
  }
  return 'bg-border-strong';
}

export function RunRail({
  run,
  running,
  onStop
}: {
  run: RunView;
  running: boolean;
  onStop: () => void;
}) {
  const [collapsed, setCollapsed] = useState(loadCollapsed);
  const listRef = useRef<HTMLOListElement>(null);
  const count = run.runs.length;
  const upcoming = useMemo(() => upcomingSteps(run), [run]);

  useEffect(() => {
    saveCollapsed(collapsed);
  }, [collapsed]);

  useEffect(() => {
    const element = listRef.current;
    if (!element) return;
    const current = element.querySelector('[data-current]');
    if (current) current.scrollIntoView({ block: 'nearest' });
    else element.scrollTop = element.scrollHeight;
  }, [count, collapsed]);


  if (collapsed) {
    return (
      <aside aria-label="Workflow" className="flex min-h-0 w-10 shrink-0 flex-col border-r border-border bg-panel">
        <div className="flex items-center justify-center p-1">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-label="Déplier"
            onClick={() => setCollapsed(false)}
          >
            <ChevronsRightIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
        <div className="flex min-h-0 grow flex-col items-center gap-2 overflow-y-auto py-2 [scrollbar-width:thin]">
          {run.runs.map((nodeRun, index) => (
            <span
              key={`${nodeRun.nodeId}-${nodeRun.pass}-${index}`}
              title={
                nodeRun.state === 'interrupted'
                  ? `${nodeRun.label} — interrompu`
                  : `${nodeRun.label} — ${nodeRun.port || nodeRun.state}`
              }
              className={`h-2 w-2 shrink-0 rounded-full ${dotClass(nodeRun)}`}
            />
          ))}
          {upcoming.map((step) => (
            <span
              key={`upcoming-${step.nodeId}`}
              title={`${step.label} — à venir`}
              className="h-2 w-2 shrink-0 rounded-full border border-dashed border-faint"
            />
          ))}
        </div>
        {!run.finished && !running && (
          <div className="flex justify-center border-t border-border p-1">
            <Button type="button" variant="danger" size="sm" iconOnly aria-label="Arrêter le workflow" title="Arrêter le workflow" onClick={onStop}>
              <StopIcon className="h-3 w-3" />
            </Button>
          </div>
        )}
      </aside>
    );
  }

  return (
    <aside aria-label="Workflow" className="flex min-h-0 w-56 shrink-0 flex-col border-r border-border bg-panel">
      <div className="flex items-center gap-1 px-2 py-2">
        <span className="min-w-0 truncate text-xs font-medium text-text" title={run.workflow.name}>
          {run.workflow.name}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          iconOnly
          aria-label="Replier"
          onClick={() => setCollapsed(true)}
          className="ml-auto"
        >
          <ChevronsLeftIcon className="h-3.5 w-3.5" />
        </Button>
      </div>
      <ol ref={listRef} className="min-h-0 grow space-y-0.5 overflow-y-auto px-2 pb-2 [scrollbar-width:thin]">
        {run.runs.map((nodeRun, index) => {
          const isRunning = nodeRun.state === 'running';
          const isDone = nodeRun.state === 'done' || nodeRun.state === 'ended';
          const isInterrupted = nodeRun.state === 'interrupted';
          const rowClass = isRunning
            ? 'rounded-md bg-accent-soft text-accent-strong'
            : isDone
              ? 'text-text'
              : 'text-muted';
          const hasTrailing = nodeRun.pass > 1 || isInterrupted || (isDone && Boolean(nodeRun.port));
          return (
            <li
              key={`${nodeRun.nodeId}-${nodeRun.pass}-${index}`}
              data-current={isRunning ? 'true' : undefined}
              className={`flex items-center gap-2 px-2 py-1 text-xs ${rowClass}`}
            >
              <span className={`h-2 w-2 shrink-0 rounded-full ${dotClass(nodeRun)}`} />
              <span
                className="min-w-0 truncate"
                title={nodeRun.state === 'interrupted' ? `${nodeRun.label} — interrompu` : nodeRun.label}
              >
                {nodeRun.label}
              </span>
              {hasTrailing && (
                <span className="ml-auto flex shrink-0 items-center gap-1">
                  {nodeRun.pass > 1 && (
                    <span className="text-[0.625rem] text-muted tabular-nums">×{nodeRun.pass}</span>
                  )}
                  {isInterrupted && <span className="text-[0.625rem] text-muted">interrompu</span>}
                  {isDone && nodeRun.port && <PortBadge port={nodeRun.port} />}
                </span>
              )}
            </li>
          );
        })}
        {upcoming.map((step) => (
          <li
            key={`upcoming-${step.nodeId}`}
            className="flex items-center gap-2 px-2 py-1 text-xs text-faint"
          >
            <span className="h-2 w-2 shrink-0 rounded-full border border-dashed border-faint" />
            <span className="min-w-0 truncate" title={`${step.label} — à venir`}>
              {step.label}
            </span>
          </li>
        ))}
      </ol>
      {(run.finished || !running) && (
        <div className="mt-auto border-t border-border p-2">
          {run.finished && (
            <Badge tone={outcomeTone(run.finished.outcome)}>{outcomeLabel(run.finished.outcome)}</Badge>
          )}
          {!run.finished && !running && (
            <Button type="button" variant="danger" size="sm" onClick={onStop} className="w-full">
              Arrêter le workflow
            </Button>
          )}
        </div>
      )}
    </aside>
  );
}
