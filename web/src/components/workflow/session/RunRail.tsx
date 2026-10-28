/**
 * The workflow run as a collapsible left rail: vertical, so every step stays visible.
 */
import { useEffect, useMemo, useRef, useState } from 'react';
import { portTone, upcomingSteps, type NodeRun, type RunView, type UpcomingStep } from '../../../lib/workflowRun';
import { formatClock, formatDuration } from '../../../lib/format';
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

/** The circle already says pass or fail; the port's name says which way (Go, minor, findings…). */
const PLAIN_PORTS = new Set(['next', 'done']);

function PortLabel({ port }: { port: string }) {
  if (PLAIN_PORTS.has(port.trim().toLowerCase())) return null;
  return <span className="shrink-0 text-[0.625rem] text-muted">{port}</span>;
}

function outcomeLabel(outcome: string): string {
  switch (outcome) {
    case 'done': return 'terminé';
    case 'blocked': return 'bloqué';
    case 'stopped': return 'arrêté';
    case 'error': return 'en erreur';
    default: return outcome;
  }
}

function outcomeTone(outcome: string): 'ok' | 'err' | 'neutral' {
  switch (outcome) {
    case 'done': return 'ok';
    case 'blocked':
    case 'stopped':
    case 'error': return 'err';
    default: return 'neutral';
  }
}

function dotClass(nodeRun: NodeRun): string {
  if (nodeRun.state === 'running') return 'animate-pulse bg-accent';
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

function circleFor(step: RailStep): { className: string; icon: 'check' | 'close' | null } {
  if (step.kind === 'upcoming') return { className: 'border border-border-strong bg-transparent', icon: null };
  const { state, port } = step.run;
  if (state === 'running') return { className: 'border-2 border-accent bg-accent-soft animate-pulse', icon: null };
  if (state === 'interrupted') return { className: 'bg-border-strong', icon: null };
  if (state === 'ended') return port === 'done' ? { className: 'bg-ok', icon: 'check' } : { className: 'bg-err', icon: 'close' };
  const tone = portTone(port ?? '');
  return tone === 'negative' ? { className: 'bg-err', icon: 'close' } : { className: 'bg-ok', icon: 'check' };
}

function connectorClass(step: RailStep, hasNext: boolean): string {
  if (!hasNext || step.kind !== 'run') return 'bg-border';
  const { state, port } = step.run;
  if (state === 'done' || state === 'ended') {
    if (state === 'ended') return port === 'done' ? 'bg-ok' : 'bg-err/60';
    return portTone(port ?? '') === 'negative' ? 'bg-err/60' : 'bg-ok';
  }
  return 'bg-border';
}

function StepRow({ step, index, isLast }: { step: RailStep; index: number; isLast: boolean }) {
  const circle = circleFor(step);
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
  const labelClass = isRunning ? 'font-semibold text-accent-strong' : isFinished ? 'text-text' : isInterrupted ? 'text-muted' : 'text-faint';
  return (
    <li data-current={isRunning ? 'true' : undefined} className="flex gap-2.5">
      <div className="flex w-4 shrink-0 flex-col items-center">
        <span className={`mt-0.5 h-4 w-4 shrink-0 rounded-full ${circle.className} ${circle.icon ? 'flex items-center justify-center' : ''}`}>
          {circle.icon === 'check' && <CheckIcon className="h-2.5 w-2.5 text-bg" />}
          {circle.icon === 'close' && <CloseIcon className="h-2.5 w-2.5 text-bg" />}
        </span>
        {!isLast && <span aria-hidden className={`w-0.5 grow min-h-3 rounded-full ${connectorClass(step, true)}`} />}
      </div>
      <div className={`min-w-0 grow ${isLast ? '' : 'pb-3'}`}>
        <span className="text-[0.5625rem] font-medium uppercase tracking-[0.06em] text-faint">
          Étape {index + 1}
          {pass > 1 && <> · ×{pass}</>}
          {isInterrupted && <> · interrompu</>}
          {time && <span className="tabular-nums"> · {time}</span>}
        </span>
        <span className="mt-0.5 flex items-center gap-1.5">
          <span className={`block min-w-0 grow truncate text-xs ${labelClass}`} title={title}>{label}</span>
          {isFinished && port ? <PortLabel port={port} /> : null}
        </span>
      </div>
    </li>
  );
}

export function RunRail({ run, running, onStop }: { run: RunView; running: boolean; onStop: () => void }) {
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
        <span className="min-w-0 truncate text-xs font-medium text-text" title={run.workflow.name}>{run.workflow.name}</span>
        <Button type="button" variant="ghost" size="sm" iconOnly aria-label="Replier" onClick={() => setCollapsed(true)} className="ml-auto">
          <ChevronsLeftIcon className="h-3.5 w-3.5" />
        </Button>
      </div>
      <ol ref={listRef} className="min-h-0 grow overflow-y-auto px-3 pt-1 pb-2 [scrollbar-width:thin]">
        {steps.map((step, index) => <StepRow key={step.key} step={step} index={index} isLast={index === steps.length - 1} />)}
      </ol>
      {(run.finished || !running) && (
        <div className="mt-auto border-t border-border p-2">
          {run.finished && <Badge tone={outcomeTone(run.finished.outcome)}>{outcomeLabel(run.finished.outcome)}</Badge>}
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
