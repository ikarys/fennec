import { modelShortName } from './ModelPicker';
import { memo, useMemo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { AssistantItem, PreflightResultView, Segment, ThreadItem } from '../../lib/fold';
import { useDisplayPrefs } from '../../lib/display';
import { splitContract, type Plan, type Report } from '../../lib/plan';
import { BulbIcon, ChevronRightIcon, ReplayIcon } from '../ui/Icons';
import { buttonClass } from '../ui/Button';
import { Card } from '../ui/Card';
import { CopyButton } from './CopyButton';
import { useCallUsage } from './CallUsage';
import { MessageStatsLine } from './MessageStats';
import { PlanCard, ReportCard } from './PlanCard';
import { ToolCard } from './ToolCard';
import { NodeDivider } from '../workflow/session/NodeDivider';
import { RunEndCard } from '../workflow/session/RunEndCard';
import { markdownComponents } from './markdownComponents';
import { splitSources } from '../../lib/sources';
import { attachmentAnchorId } from '../../lib/vision';
import { planAnchorId, usePendingPlan } from './PendingPlan';
import { useT } from '../../i18n';
import type { SessionStatus } from '../../lib/sessionStatus';

// Memoised on their text: while one message streams, the others are not
// parsed again on every frame.
const ThinkingSegment = memo(function ThinkingSegment({ content }: { content: string }) {
  const t = useT();
  const display = useDisplayPrefs();
  const [open, setOpen] = useState(display.reasoningOpen);
  if (!content) return null;
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex min-h-8 items-center gap-2 self-start rounded-lg border border-dashed border-info/50 bg-transparent px-2.5 text-sm text-muted"
      >
        <BulbIcon className="h-3.5 w-3.5 text-info" />
        {t(open ? 'thinkingHide' : 'thinkingShow')}
      </button>
      {open && (
        <div className="border-l-2 border-info/50 pl-3.5 text-sm whitespace-pre-line text-muted">{content}</div>
      )}
    </div>
  );
});

const REMARK_PLUGINS = [remarkGfm];

const TextSegment = memo(function TextSegment({ content, caret, preflight }: { content: string; caret: boolean; preflight?: PreflightResultView[] }) {
  const t = useT();
  // Parsed only when settled: a half-written contract would flicker between the
  // two renderings while the message streams.
  const contract = useMemo(() => (caret ? null : splitContract(content)), [content, caret]);
  const split = useMemo(() => splitSources(content), [content]);
  // The model's words sit on a surface, like the plan card, so they are told
  // apart from the thread's chrome at a glance. A plan or report card is itself
  // a Card: it renders bare, the leading prose is surfaced.
  if (contract) {
    return (
      <>
        {contract.before && (
          <Card className="px-3.5 py-2.5">
            <div className="min-w-0 text-sm leading-relaxed [overflow-wrap:break-word]">
              <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                {contract.before}
              </ReactMarkdown>
            </div>
          </Card>
        )}
        {contract.contract.kind === 'plan' ? <PlanCard plan={contract.contract} preflight={preflight} /> : <ReportCard report={contract.contract} />}
      </>
    );
  }
  const { body, sources } = split;
  return (
    <Card className="px-3.5 py-2.5">
      <div className="min-w-0 text-sm leading-relaxed [overflow-wrap:break-word]">
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{body}</ReactMarkdown>
        {caret && <span className="caret ml-0.5 inline-block h-4 w-2 bg-accent align-text-bottom" />}
        {sources !== null && (
          <details className="group mt-2 border-t border-border pt-2">
            <summary className="flex cursor-pointer list-none items-center gap-2 rounded-md px-1.5 py-1 text-sm text-muted select-none hover:bg-accent-soft hover:text-text [&::-webkit-details-marker]:hidden">
              <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform group-open:rotate-90" />
              {t('answerSources')}
            </summary>
            <div className="pl-5"><ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{sources}</ReactMarkdown></div>
          </details>
        )}
      </div>
    </Card>
  );
});

// The plan a plan agent recorded with propose_plan: the same card a text
// contract renders. Its actions live in the decision bar; the card stays the record.
const PlanSegment = memo(function PlanSegment({ id, plan, preflight }: { id: string; plan: Plan; preflight?: PreflightResultView[] }) {
  const pending = usePendingPlan() === id;
  return <PlanCard plan={plan} preflight={preflight} pending={pending} anchorId={planAnchorId(id)} />;
});

// The report a builder recorded with submit_report: the same card a text
// contract renders.
const ReportSegment = memo(function ReportSegment({ report }: { report: Report }) {
  return <ReportCard report={report} />;
});

// A centered note that carries a detail (a compaction, a replay): the detail
// opens under it on the text's own surface, never a box inside a box.
const NoteDetail = memo(function NoteDetail({ text, detail }: { text: string; detail: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className={buttonClass({ variant: 'ghost', size: 'sm' }, 'self-center')}
      >
        <ChevronRightIcon className={`h-3 w-3 transition-transform ${open ? 'rotate-90' : ''}`} />
        {text}
      </button>
      {open && <TextSegment content={detail} caret={false} />}
    </div>
  );
});

function AssistantItemView({ item, model, running, showHeader, pendingCallId }: { item: AssistantItem; model: string; running: boolean; showHeader: boolean; pendingCallId?: string }) {
  const t = useT();
  const lastTextId = [...item.segments].reverse().find((segment): segment is Extract<Segment, { kind: 'text' }> => segment.kind === 'text')?.id;
  const streaming = running && !item.done;
  const copyText = item.done
    ? item.segments
        .map((segment) => (segment.kind === 'text' ? segment.content : segment.kind === 'plan' || segment.kind === 'report' ? segment.markdown : ''))
        .filter((text) => text.length > 0)
        .join('\n\n')
    : '';
  const showCopy = item.done && copyText.length > 0;
  const call = useCallUsage(item.id);
  const showStats = streaming ? item.liveSpeed != null : call != null;

  return (
    <div className="flex flex-col gap-2.5">
      {showHeader && (
        <div className="flex items-center gap-2 text-xs text-muted">
          {/* The model that answered, from the ledger; the session's current one while it streams. */}
          <span className="font-semibold text-text">{call ? modelShortName(call.model) : model}</span>
          <span className="rounded-md border border-border px-1.5 text-xs">{t('assistantProducedByModel')}</span>
        </div>
      )}
      {item.segments.map((segment) => {
        if (segment.kind === 'thinking') {
          return <ThinkingSegment key={segment.id} content={segment.content} />;
        }
        if (segment.kind === 'text') {
          return <TextSegment key={segment.id} content={segment.content} caret={streaming && segment.id === lastTextId} preflight={segment.id === lastTextId ? item.preflight : undefined} />;
        }
        if (segment.kind === 'plan') {
          return <PlanSegment key={segment.id} id={segment.id} plan={segment.plan} preflight={item.preflight} />;
        }
        if (segment.kind === 'report') {
          return <ReportSegment key={segment.id} report={segment.report} />;
        }
        return <ToolCard key={segment.id} tool={segment.tool} awaiting={segment.tool.id === pendingCallId} />;
      })}
      {/* The copy button follows the stats line, so it stays by the answer whatever its width. */}
      {(showStats || showCopy) && (
        <div className="flex items-center gap-1">
          <MessageStatsLine item={item} streaming={streaming} />
          {showCopy && <CopyButton text={copyText} />}
        </div>
      )}
      {!streaming && call?.turnRecap && <div className="text-center text-xs text-muted tabular-nums">{call.turnRecap}</div>}
    </div>
  );
}

export function ThreadItemView({
  item,
  model,
  running,
  showHeader,
  onReplay,
  pendingCallId,
  waitingStatus,
}: {
  item: ThreadItem;
  model: string;
  running: boolean;
  showHeader: boolean;
  onReplay?: (messageId: string) => void;
  pendingCallId?: string;
  waitingStatus?: SessionStatus;
}) {
  const t = useT();
  if (item.kind === 'mode') {
    // A mode change is a divider, not a message: the agent's color marks the
    // switch without taking bubble space.
    const color = item.color;
    const textStyle = color ? { color } : undefined;
    const lineStyle = color ? { backgroundColor: color } : undefined;
    return (
      <div className="flex items-center gap-2 text-xs text-muted">
        <span className="h-px grow bg-border" style={lineStyle} />
        <span style={textStyle}>{item.name} {t('threadMode')}</span>
        <span className="h-px grow bg-border" style={lineStyle} />
      </div>
    );
  }

  if (item.kind === 'user') {
    if (item.isSystemGenerated) {
      // fennec's own prompts to the model (compaction, resume, continue): the
      // thread keeps the `context.compacted` line, not their text.
      return null;
    }
    return (
      <div className="group flex max-w-[85%] flex-col self-end">
        <div className="rounded-xl rounded-br-sm border border-accent/30 bg-accent-soft px-3.5 py-2.5 [overflow-wrap:break-word]">
          {item.attachments.length > 0 && (
            <div className="flex flex-wrap gap-2">
              {item.attachments.map((a) => (
                <img key={a.id} id={attachmentAnchorId(a.id)} src={a.data} alt={a.filename} className="max-h-48 rounded-lg border border-border" />
              ))}
            </div>
          )}
          {item.content && <div className="whitespace-pre-wrap">{item.content}</div>}
        </div>
        <div className="mt-1 flex justify-end gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          <CopyButton text={item.content} />
          {onReplay && (
            <button
              type="button"
              aria-label={t('threadReplay')}
              title={t('threadReplay')}
              disabled={running}
              onClick={() => onReplay(item.id)}
              className="rounded-md p-1 text-muted transition-colors hover:text-text disabled:opacity-60"
            >
              <ReplayIcon className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      </div>
    );
  }

  if (item.kind === 'assistant') {
    return <AssistantItemView item={item} model={model} running={running} showHeader={showHeader} pendingCallId={pendingCallId} />;
  }

  if (item.kind === 'error') {
    return <div className="rounded-md border border-border bg-raised px-3 py-2 text-sm text-err">{item.message}</div>;
  }

  if (item.kind === 'note') {
    if (item.detail !== undefined) {
      return <NoteDetail text={item.text} detail={item.detail} />;
    }
    return <div className="text-center text-xs text-muted">{item.text}</div>;
  }

  if (item.kind === 'workflow-node') {
    return <NodeDivider item={item} running={running} waitingStatus={waitingStatus} />;
  }

  if (item.kind === 'workflow-end') {
    return <RunEndCard item={item} />;
  }
}
