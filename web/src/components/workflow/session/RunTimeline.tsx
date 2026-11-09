// The workflow run's drawer body: a summary and a vertical step timeline.
import { Fragment, useEffect, useRef, useState, type ReactNode } from 'react';
import { portTone, runRounds, upcomingSteps, type NodeRun, type RunView } from '../../../lib/workflowRun';
import { formatClock, formatDuration } from '../../../lib/format';
import { useLang, useT } from '../../../i18n';
import { NODE_KIND_LABELS } from '../../../lib/workflow';
import { Badge } from '../../ui/Badge';
import { CheckIcon, ChevronDownIcon, ChevronRightIcon, ClockIcon, CloseIcon, PauseIcon, SpinnerIcon } from '../../ui/Icons';
import { NodeOutputModal } from './NodeOutputModal';
import { isPlainPort, nodeRunTone, STEP_MARKER_CLASS, stepTone, type RailStep, type StepTone } from './runStepTone';

function Marker({ tone }: { tone: StepTone }) {
  const className = `grid size-5 place-items-center rounded-full ${STEP_MARKER_CLASS[tone]}`;

  if (tone === 'ok') return <span className={className}><CheckIcon className="h-3 w-3" /></span>;
  if (tone === 'err') return <span className={className}><CloseIcon className="h-3 w-3" /></span>;
  if (tone === 'running') return <span className={className}><SpinnerIcon className="h-3 w-3 animate-spin" /></span>;
  if (tone === 'waiting') return <span className={className}><ClockIcon className="h-3 w-3" /></span>;
  if (tone === 'interrupted') return <span className={className}><PauseIcon className="h-3 w-3" /></span>;
  return <span className={className} />;
}

function StepRow({ step, isLast, onShowOutput }: { step: RailStep; isLast: boolean; onShowOutput: (run: NodeRun) => void }) {
  const lang = useLang();
  const t = useT();
  const tone = stepTone(step);
  const nodeRun = step.kind === 'run' ? step.run : null;
  const upcoming = step.kind === 'upcoming' ? step.step : null;
  const label = nodeRun ? nodeRun.label : upcoming?.label ?? '';
  const isRunning = nodeRun?.state === 'running';
  const isHumanRunning = isRunning && nodeRun?.kind === 'human';
  const isInterrupted = nodeRun?.state === 'interrupted';
  const isFinished = nodeRun?.state === 'done' || nodeRun?.state === 'ended';
  const port = nodeRun?.port;
  const pass = nodeRun?.pass ?? 1;

  const [elapsedMs, setElapsedMs] = useState(0);
  useEffect(() => {
    if (!isRunning || isHumanRunning) return;
    const start = nodeRun?.startedAt ?? Date.now();
    setElapsedMs(Date.now() - start);
    const interval = window.setInterval(() => setElapsedMs(Date.now() - start), 1000);
    return () => window.clearInterval(interval);
  }, [isRunning, isHumanRunning, nodeRun?.startedAt]);

  const time = isHumanRunning
    ? t('awaiting')
    : isRunning
      ? formatClock(elapsedMs)
      : nodeRun?.durationMs != null
        ? formatDuration(lang, nodeRun.durationMs)
        : nodeRun?.state === 'ended' && elapsedMs > 0
          ? formatClock(elapsedMs)
          : null;

  const labelClass = isRunning ? 'font-medium text-text' : isFinished ? 'text-text' : isInterrupted ? 'text-muted' : 'text-faint';
  const title = nodeRun ? (isInterrupted ? t('runRailInterrupted', { label }) : label) : t('runRailUpcoming', { label });
  const outputLine = nodeRun?.output
    ?.split('\n')
    .map((line) => line.trim())
    .find((line) => line.length > 0 && !line.startsWith('#'))
    ?.replace(/[*_`]/g, '');

  const showPort = isFinished && !!port && !isPlainPort(port);
  const kind = nodeRun?.kind;
  const showKind = kind === 'human' || kind === 'criteria' || kind === 'shell';
  const hasMeta = showPort || pass > 1 || time !== null || showKind;

  const portToneValue = port ? portTone(port) : 'neutral';
  const portBadge = port ? (
    <Badge appearance="pill" tone={portToneValue === 'positive' ? 'ok' : portToneValue === 'negative' ? 'err' : 'neutral'}>
      {port}
    </Badge>
  ) : null;

  return (
    <li data-current={isRunning ? 'true' : undefined} className="relative flex gap-2.5 pb-3 last:pb-0">
      {!isLast ? <span aria-hidden className="absolute left-[9px] top-6 bottom-0 w-px bg-border" /> : null}
      <Marker tone={tone} />
      <div className={`min-w-0 grow ${isRunning ? 'rounded-md bg-accent-soft/40 -mx-1.5 px-1.5 py-0.5' : ''}`}>
        <div className="flex items-baseline gap-2">
          <span className={`truncate text-sm ${labelClass}`} title={title}>
            {label}
          </span>
          {time ? <span className="ml-auto shrink-0 text-xs text-muted tabular-nums">{time}</span> : null}
        </div>
        {hasMeta ? (
          <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
            {showKind && nodeRun ? <span className="text-[11px] text-faint">{t(NODE_KIND_LABELS[nodeRun.kind as keyof typeof NODE_KIND_LABELS])}</span> : null}
            {showPort && portBadge ? (
              nodeRun?.output ? (
                <button
                  type="button"
                  title={t('runRailViewOutput')}
                  onClick={() => {
                    if (nodeRun) onShowOutput(nodeRun);
                  }}
                  className="cursor-pointer"
                >
                  {portBadge}
                </button>
              ) : (
                portBadge
              )
            ) : null}
            {pass > 1 ? <span className="text-xs text-muted tabular-nums">×{pass}</span> : null}
          </div>
        ) : null}
        {tone === 'err' && outputLine ? (
          <span className="mt-0.5 block truncate text-xs text-err/80" title={nodeRun?.output ?? ''}>
            {outputLine}
          </span>
        ) : null}
      </div>
    </li>
  );
}

function RoundRow({
  index,
  round,
  expanded,
  isLast,
  onToggle,
  onShowOutput,
}: {
  index: number;
  round: NodeRun[];
  expanded: boolean;
  isLast: boolean;
  onToggle: (index: number) => void;
  onShowOutput: (run: NodeRun) => void;
}) {
  const lang = useLang();
  const t = useT();
  const last = round[round.length - 1];
  if (!last) return null;

  const tone = nodeRunTone(last);
  const hasDuration = round.some((nodeRun) => nodeRun.durationMs != null);
  const totalDuration = round.reduce((total, nodeRun) => total + (nodeRun.durationMs ?? 0), 0);

  return (
    <li className="relative flex gap-2.5 pb-3 last:pb-0">
      {!isLast ? <span aria-hidden className="absolute left-[9px] top-6 bottom-0 w-px bg-border" /> : null}
      <Marker tone={tone} />
      <div className="min-w-0 grow">
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => onToggle(index)}
          className="-mx-1.5 flex w-[calc(100%+0.75rem)] cursor-pointer items-center gap-2 rounded-md px-1.5 py-0.5 text-left hover:bg-raised"
        >
          {expanded ? <ChevronDownIcon className="h-3.5 w-3.5 shrink-0 text-muted" /> : <ChevronRightIcon className="h-3.5 w-3.5 shrink-0 text-muted" />}
          <span className="truncate text-sm text-text">{t('runRailRound', { index: index + 1 })}</span>
          <span className="ml-auto flex shrink-0 items-baseline gap-1.5 text-xs text-muted tabular-nums">
            <span>{t('runRailStepCount', { count: round.length, plural: round.length > 1 ? 's' : '' })}</span>
            {hasDuration ? <span className="text-faint">{formatDuration(lang, totalDuration)}</span> : null}
          </span>
        </button>
        {expanded ? (
          <ol className="mt-1 flex flex-col">
            {round.map((nodeRun, runIndex) => (
              <StepRow
                key={`${nodeRun.nodeId}-${nodeRun.pass}-${runIndex}`}
                step={{ kind: 'run' as const, run: nodeRun }}
                isLast={runIndex === round.length - 1}
                onShowOutput={onShowOutput}
              />
            ))}
          </ol>
        ) : null}
      </div>
    </li>
  );
}

export function RunTimeline({ run }: { run: RunView }) {
  const t = useT();
  const rootRef = useRef<HTMLDivElement>(null);
  const [expandedRounds, setExpandedRounds] = useState<Set<number>>(new Set());
  const [outputRun, setOutputRun] = useState<NodeRun | null>(null);

  const rounds = runRounds(run.runs);
  const upcoming = upcomingSteps(run);
  const count = run.runs.length;

  useEffect(() => {
    const current = rootRef.current?.querySelector('[data-current]');
    if (current) current.scrollIntoView({ block: 'nearest' });
  }, [count]);

  const toggleRound = (index: number): void => {
    setExpandedRounds((previous) => {
      const next = new Set(previous);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const finishedCount = run.runs.filter((nodeRun) => nodeRun.state === 'done' || nodeRun.state === 'ended').length;
  const openCount = run.runs.filter((nodeRun) => nodeRun.state === 'running').length;
  const total = finishedCount + openCount + upcoming.length;
  const outcome = run.finished?.outcome;

  const stateTone = outcome === 'done' ? 'ok' : outcome === 'blocked' || outcome === 'error' ? 'err' : outcome === 'stopped' ? 'neutral' : 'accent';
  const stateLabel =
    outcome === 'done'
      ? t('runRailStateDone')
      : outcome === 'blocked' || outcome === 'error'
        ? t('runRailStateFailed')
        : outcome === 'stopped'
          ? t('runRailStateStopped')
          : t('runRailStateRunning');

  type TimelineRow = { key: string; render: (isLast: boolean) => ReactNode };
  const rows: TimelineRow[] = [];

  rounds.slice(0, -1).forEach((round, index) => {
    rows.push({
      key: `round-${index}`,
      render: (isLast) => (
        <RoundRow
          index={index}
          round={round}
          expanded={expandedRounds.has(index)}
          isLast={isLast}
          onToggle={toggleRound}
          onShowOutput={setOutputRun}
        />
      ),
    });
  });

  if (rounds.length > 0) {
    const lastRound = rounds[rounds.length - 1]!;
    lastRound.forEach((nodeRun, index) => {
      rows.push({
        key: `run-${nodeRun.nodeId}-${nodeRun.pass}-${index}`,
        render: (isLast) => (
          <StepRow
            step={{ kind: 'run' as const, run: nodeRun }}
            isLast={isLast}
            onShowOutput={setOutputRun}
          />
        ),
      });
    });
  }

  upcoming.forEach((step) => {
    rows.push({
      key: `upcoming-${step.nodeId}`,
      render: (isLast) => (
        <StepRow step={{ kind: 'upcoming' as const, step }} isLast={isLast} onShowOutput={setOutputRun} />
      ),
    });
  });

  return (
    <div ref={rootRef} className="flex min-h-0 flex-col">
      <header className="border-b border-border px-3 py-2.5">
        <div className="truncate text-sm font-semibold text-text">{run.workflow.name}</div>
        <div className="mt-1 flex items-center gap-2 text-xs">
          <Badge appearance="pill" tone={stateTone}>
            {stateLabel}
          </Badge>
          <span className="tabular-nums text-muted">
            {finishedCount}/{total}
          </span>
        </div>
      </header>
      <ol className="flex flex-col px-3 py-2">
        {rows.map((row, index) => (
          <Fragment key={row.key}>{row.render(index === rows.length - 1)}</Fragment>
        ))}
      </ol>
      <NodeOutputModal nodeRun={outputRun} onClose={() => setOutputRun(null)} />
    </div>
  );
}
