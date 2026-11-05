import { useEffect, useRef, useState } from 'react';
import type { ThreadItem } from '../../../lib/fold';
import { formatClock, formatDuration } from '../../../lib/format';
import type { NodeKind } from '../../../lib/workflow';
import { useLang, useT } from '../../../i18n';
import { portTone } from '../../../lib/workflowRun';
import { CheckIcon, ChevronRightIcon, CloseIcon, ExternalIcon, SpinnerIcon } from '../../Icons';
import { useNodeUsage } from '../../session/CallUsage';
import { Badge } from '../../ui/Badge';
import { buttonClass } from '../../ui/Button';
import { SectionLabel } from '../../ui/SectionLabel';

type WorkflowNodeItem = Extract<ThreadItem, { kind: 'workflow-node' }>;

function portBadgeTone(port: string): 'ok' | 'err' | 'accent' {
  const tone = portTone(port);
  if (tone === 'positive') return 'ok';
  if (tone === 'negative') return 'err';
  return 'accent';
}

/** The node kind's text colour by role, following the workflow editor's semantics. */
function nodeKindTone(kind: NodeKind): string {
  switch (kind) {
    case 'agent':
      return 'text-muted';
    case 'human':
      return 'text-accent';
    case 'criteria':
      return 'text-ok';
    default:
      return 'text-muted';
  }
}

export function NodeDivider({ item, running }: { item: WorkflowNodeItem; running: boolean }) {
  const lang = useLang();
  const t = useT();
  const finished = item.port !== undefined;
  const usage = useNodeUsage(item.id);
  const badge = finished ? { port: item.port!, tone: portBadgeTone(item.port!) } : null;
  const hasDetails = Boolean(item.prompt || item.output || item.childSessionId);
  const [open, setOpen] = useState(false);
  // Elapsed since the node started; logs written before startedAt was carried
  // fall back to the mount time.
  const startRef = useRef<number | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);

  useEffect(() => {
    if (!running || finished) return;
    const start = item.startedAt ?? (startRef.current ?? (startRef.current = Date.now()));
    setElapsedMs(Date.now() - start);
    const interval = window.setInterval(() => {
      setElapsedMs(Date.now() - start);
    }, 1000);
    return () => window.clearInterval(interval);
  }, [running, finished, item.startedAt]);

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-xs text-muted">
        <span className="h-px grow bg-border" />
        <span className="flex min-w-0 items-center gap-1.5">
          <span
            className={`shrink-0 text-xs ${nodeKindTone(item.nodeKind)}`}
          >
            {item.nodeKind}
          </span>
          <span className="truncate text-text" title={item.label}>
            {item.label}
          </span>
          {item.pass > 1 && <Badge tone="neutral">{t('nodeDividerPassage', { pass: item.pass })}</Badge>}
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
            <span className="tabular-nums">{formatDuration(lang, item.durationMs)}</span>
          )}
          {finished && usage && (
            <span className="tabular-nums text-muted" title={usage.detail}>
              {usage.line}
            </span>
          )}
          {!finished && running && (
            <span className="flex items-center gap-1 text-accent-strong">
              <SpinnerIcon className="h-3 w-3" />
              {t('nodeDividerRunning')}
              <span className="tabular-nums">{formatClock(elapsedMs)}</span>
            </span>
          )}
          {!finished && !running && <span>{t('nodeDividerInterrupted')}</span>}
        </span>
        {hasDetails ? (
          <button
            type="button"
            className={buttonClass({ variant: 'ghost', size: 'sm' }, 'ml-1')}
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            <ChevronRightIcon className={`h-3 w-3 transition-transform ${open ? 'rotate-90' : ''}`} />
            {t('nodeDividerDetails')}
          </button>
        ) : (
          <span className="h-px grow bg-border" />
        )}
      </div>
      {hasDetails && open && (
        <div className="ml-px border-l border-border pl-3">
          <div className="flex flex-col gap-2.5 rounded-md border border-border bg-raised px-3 py-2 text-left text-xs">
            {item.prompt && (
              <div className="flex flex-col gap-1">
                <SectionLabel>{t('agentFieldsPromptLabel')}</SectionLabel>
                <div className="max-h-48 overflow-y-auto whitespace-pre-wrap text-muted">{item.prompt}</div>
              </div>
            )}
            {item.output && (
              <div className="flex flex-col gap-1">
                <SectionLabel>{t('outputLabel')}</SectionLabel>
                {item.nodeKind === 'shell' || item.nodeKind === 'criteria' ? (
                  <pre className="max-h-48 overflow-y-auto whitespace-pre-wrap break-words rounded bg-code p-2 font-mono text-xs text-muted">
                    {item.output}
                  </pre>
                ) : (
                  <div className="max-h-48 overflow-y-auto whitespace-pre-wrap text-text">{item.output}</div>
                )}
                {item.nodeKind === 'shell' && item.exitCode !== undefined && (
                  <div className="flex justify-end">
                    <Badge tone={item.exitCode === 0 ? 'ok' : 'err'}>{t('nodeDividerExitCode', { code: item.exitCode })}</Badge>
                  </div>
                )}
              </div>
            )}
            {item.childSessionId && (
              <div className="flex justify-end">
                <a href={`#/s/${item.childSessionId}`} className={buttonClass({ variant: 'ghost', size: 'sm' })}>
                  <ExternalIcon className="h-3 w-3" />
                  {t('openSession')}
                </a>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
