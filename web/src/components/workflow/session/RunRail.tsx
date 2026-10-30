/**
 * The workflow run as a collapsible left rail: vertical, so every step stays visible.
 */
import { useEffect, useMemo, useRef, useState } from 'react';
import { portTone, upcomingSteps, type NodeRun, type RunView, type UpcomingStep } from '../../../lib/workflowRun';
import { formatClock, formatDuration } from '../../../lib/format';
import { ChevronsLeftIcon, ChevronsRightIcon } from '../../Icons';
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

/** The bar already says pass or fail; the port's name says which way (Go, minor, findings…). */
const PLAIN_PORTS = new Set(['next', 'done']);

function PortLabel({ port }: { port: string }) {
  if (PLAIN_PORTS.has(port.trim().toLowerCase())) return null;
  return <span className="shrink-0 text-[0.6875rem] text-muted">{port}</span>;
}


function dotClass(nodeRun: NodeRun): string {
  if (nodeRun.state === 'running') return 'bg-accent';
  if (nodeRun.state === 'interrupted') return 'bg-border-strong';
  if (nodeRun.state === 'ended') return nodeRun.port === 'done' ? 'bg-ok' : 'bg-err';
  if (nodeRun.state === 'done') {
    const tone = portTone(nodeRun.port ?? '');
    if (tone === 'positive') return 'bg-ok';
    if (tone === 'negative') return 'bg-err';
    return 'bg-muted';
  }
  return 'bg-border-strong';
}

type RailStep =
  | { kind: 'run'; run: NodeRun; key: string }
  | { kind: 'upcoming'; step: UpcomingStep; key: string };

function barClass(step: RailStep): string {
  if (step.kind === 'upcoming') return 'border-border';
  const { state, port } = step.run;
  if (state === 'running') return 'border-accent';
  if (state === 'interrupted') return 'border-border-strong';
  if (state === 'ended') return port === 'done' ? 'border-ok' : 'border-err';
  return portTone(port ?? '') === 'negative' ? 'border-err' : 'border-ok';
}

function StepRow({ step }: { step: RailStep }) {
  const bar = barClass(step);
  const label = step.kind === 'run' ? step.run.label : step.step.label;
  const nodeRun = step.kind === 'run' ? step.run : null;
  const isRunning = nodeRun?.state === 'running';
  const isInterrupted = nodeRun?.state === 'interrupted';
  const isFinished = nodeRun?.state === 'done' || nodeRun?.state === 'ended';
  const port = nodeRun?.port;
  const pass = nodeRun?.pass ?? 1;
  // Elapsed time while the node runs; logs written before startedAt was
  // carried fall back to the mount time.
  const [elapsedMs, setElapsedMs] = useState(0);
  useEffect(() => {
    if (!isRunning) return;
    const start = nodeRun?.startedAt ?? Date.now();
    setElapsedMs(Date.now() - start);
    const interval = window.setInterval(() => setElapsedMs(Date.now() - start), 1000);
    return () => window.clearInterval(interval);
  }, [isRunning, nodeRun?.startedAt]);
  // A node that ends the run has no workflow.node.finished. The frozen tick
  // only exists when the row was watched running; a late reload has none, so
  // no figure is shown (a 00:00 would be a lie).
  const time = isRunning
    ? formatClock(elapsedMs)
    : nodeRun?.durationMs != null
      ? formatDuration(nodeRun.durationMs)
      : nodeRun?.state === 'ended' && elapsedMs > 0
        ? formatClock(elapsedMs)
        : null;
  const title = step.kind === 'run' ? (isInterrupted ? `${label} — interrompu` : label) : `${label} — à venir`;
  const labelClass = isRunning
    ? 'font-medium text-text'
    : isFinished
      ? 'text-text'
      : isInterrupted
        ? 'text-muted'
        : 'text-faint';
  // The first line that says something: a report often opens with a Markdown heading.
  const outputLine = nodeRun?.output
    ?.split('\n')
    .map((line) => line.trim())
    .find((line) => line.length > 0 && !line.startsWith('#'));
  const showPort = isFinished && !!port && !PLAIN_PORTS.has(port.trim().toLowerCase());
  const hasMeta = showPort || pass > 1 || time;
  return (
    <li data-current={isRunning ? 'true' : undefined} className={`flex flex-col border-l-2 ${bar} py-1 pl-2`}>
      <span className={`truncate text-xs ${labelClass}`} title={title}>{label}</span>
      {hasMeta && (
        <span className="flex items-baseline gap-1.5 whitespace-nowrap text-[0.6875rem] text-muted">
          {showPort && port ? <PortLabel port={port} /> : null}
          {pass > 1 && <span className="tabular-nums">×{pass}</span>}
          {time && <span className="ml-auto font-mono text-faint tabular-nums">{time}</span>}
        </span>
      )}
      {bar === 'border-err' && outputLine ? (
        <span className="truncate text-[0.6875rem] text-muted" title={nodeRun?.output ?? ''}>{outputLine}</span>
      ) : null}
    </li>
  );
}

export function RunRail({ run }: { run: RunView }) {
  const [collapsed, setCollapsed] = useState(loadCollapsed);
  const listRef = useRef<HTMLOListElement>(null);
  const count = run.runs.length;
  const upcoming = useMemo(() => upcomingSteps(run), [run]);
  const steps = useMemo<RailStep[]>(() => [
    ...run.runs.map((nodeRun, index) => ({ kind: 'run' as const, run: nodeRun, key: `${nodeRun.nodeId}-${nodeRun.pass}-${index}` })),
    ...upcoming.map((step) => ({ kind: 'upcoming' as const, step, key: `upcoming-${step.nodeId}` }))
  ], [run, upcoming]);

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
          <Button type="button" variant="ghost" size="sm" iconOnly aria-label="Déplier" onClick={() => setCollapsed(false)}>
            <ChevronsRightIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
        <div className="flex min-h-0 grow flex-col items-center gap-2 overflow-y-auto py-2 [scrollbar-width:thin]">
          {run.runs.map((nodeRun, index) => (
            <span
              key={`${nodeRun.nodeId}-${nodeRun.pass}-${index}`}
              title={nodeRun.state === 'interrupted' ? `${nodeRun.label} — interrompu` : `${nodeRun.label} — ${nodeRun.port || nodeRun.state}`}
              className={`h-2 w-2 shrink-0 rounded-full ${dotClass(nodeRun)}`}
            />
          ))}
          {upcoming.map((step) => (
            <span key={`upcoming-${step.nodeId}`} title={`${step.label} — à venir`} className="h-2 w-2 shrink-0 rounded-full border border-dashed border-faint" />
          ))}
        </div>
      </aside>
    );
  }

  return (
    <aside aria-label="Workflow" className="flex min-h-0 w-56 shrink-0 flex-col border-r border-border bg-panel">
      <div className="flex items-center gap-1 px-2 py-2">
        <span className="min-w-0 truncate text-xs font-medium text-text" title={run.workflow.name}>{run.workflow.name}</span>
        <Button type="button" variant="ghost" size="sm" iconOnly aria-label="Replier" onClick={() => setCollapsed(true)} className="ml-auto">
          <ChevronsLeftIcon className="h-3.5 w-3.5" />
        </Button>
      </div>
      <ol ref={listRef} className="flex min-h-0 grow flex-col gap-0.5 overflow-y-auto px-2 pt-1 pb-2 [scrollbar-width:thin]">
        {steps.map((step) => <StepRow key={step.key} step={step} />)}
      </ol>
    </aside>
  );
}
