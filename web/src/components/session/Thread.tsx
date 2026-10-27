import { memo, useMemo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { AssistantItem, Segment, ThreadItem } from '../../lib/fold';
import { useDisplayPrefs } from '../../lib/display';
import { splitContract } from '../../lib/plan';
import { BulbIcon, ReplayIcon } from '../Icons';
import { CopyButton } from './CopyButton';
import { MessageStatsLine } from './MessageStats';
import { PlanCard, ReportCard } from './PlanCard';
import { ToolCard } from './ToolCard';
import { NodeDivider } from '../workflow/session/NodeDivider';
import { RunEndCard } from '../workflow/session/RunEndCard';
import { markdownComponents } from './markdownComponents';
import { attachmentAnchorId } from '../../lib/vision';
import { usePlanActions } from './PlanActions';



// Memoised on their text: while one message streams, the others are not
// parsed again on every frame.
const ThinkingSegment = memo(function ThinkingSegment({ content }: { content: string }) {
  const display = useDisplayPrefs();
  const [open, setOpen] = useState(display.reasoningOpen);
  if (!content) return null;
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex min-h-8 items-center gap-2 self-start rounded-lg border border-dashed border-border bg-transparent px-2.5 text-[0.8125rem] text-muted"
      >
        <BulbIcon className="h-3.5 w-3.5" />
        Réflexion · {open ? 'masquer' : 'afficher'}
      </button>
      {open && (
        <div className="border-l-2 border-border pl-3.5 text-[0.8125rem] whitespace-pre-line text-muted">{content}</div>
      )}
    </div>
  );
});

const REMARK_PLUGINS = [remarkGfm];

const TextSegment = memo(function TextSegment({ content, caret, id }: { content: string; caret: boolean; id?: string }) {
  const actions = usePlanActions();
  // Parsed only when settled: a half-written contract would flicker between the
  // two renderings while the message streams.
  const contract = useMemo(() => (caret ? null : splitContract(content)), [content, caret]);
  const planActions =
    actions && id !== undefined && actions.segmentId === id && contract?.contract.kind === 'plan'
      ? actions
      : null;
  return (
    <div className="min-w-0 text-[0.875rem] leading-relaxed [overflow-wrap:break-word]">
      {contract ? (
        <>
          {contract.before && (
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {contract.before}
            </ReactMarkdown>
          )}
          {contract.contract.kind === 'plan' ? <PlanCard plan={contract.contract} actions={planActions} /> : <ReportCard report={contract.contract} />}
        </>
      ) : (
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{content}</ReactMarkdown>
      )}
      {caret && <span className="caret ml-0.5 inline-block h-4 w-2 bg-accent align-text-bottom" />}
    </div>
  );
});

function AssistantItemView({ item, model, running, showHeader }: { item: AssistantItem; model: string; running: boolean; showHeader: boolean }) {
  const lastTextId = [...item.segments].reverse().find((segment): segment is Extract<Segment, { kind: 'text' }> => segment.kind === 'text')?.id;
  const streaming = running && !item.done;
  const copyText = item.done
    ? item.segments
        .filter((segment): segment is Extract<Segment, { kind: 'text' }> => segment.kind === 'text')
        .map((segment) => segment.content)
        .join('\n\n')
    : '';
  const showCopy = item.done && copyText.length > 0;
  const showStats = streaming ? item.liveSpeed != null : item.stats != null;

  return (
    <div className="flex flex-col gap-2.5">
      {showHeader && (
        <div className="flex items-center gap-2 text-xs text-muted">
          <span className="font-semibold text-text">{model}</span>
          <span className="rounded border border-border px-1.5 text-[0.6875rem]">produit par le modèle</span>
        </div>
      )}
      {item.segments.map((segment) => {
        if (segment.kind === 'thinking') {
          return <ThinkingSegment key={segment.id} content={segment.content} />;
        }
        if (segment.kind === 'text') {
          return <TextSegment key={segment.id} id={segment.id} content={segment.content} caret={streaming && segment.id === lastTextId} />;
        }
        return <ToolCard key={segment.id} tool={segment.tool} />;
      })}
      {showCopy && showStats ? (
        <div className="flex items-center justify-between gap-3">
          <MessageStatsLine item={item} streaming={streaming} />
          <CopyButton text={copyText} />
        </div>
      ) : (
        <>
          <MessageStatsLine item={item} streaming={streaming} />
          {showCopy && (
            <div className="flex justify-end">
              <CopyButton text={copyText} />
            </div>
          )}
        </>
      )}
    </div>
  );
}

export function ThreadItemView({ item, model, running, showHeader, onReplay }: { item: ThreadItem; model: string; running: boolean; showHeader: boolean; onReplay?: (messageId: string) => void }) {
  if (item.kind === 'mode') {
    // A mode change is a divider, not a message: the agent's color marks the
    // switch without taking bubble space.
    const color = item.color;
    const textStyle = color ? { color } : undefined;
    const lineStyle = color ? { backgroundColor: color } : undefined;
    return (
      <div className="flex items-center gap-2 text-[0.6875rem] text-muted">
        <span className="h-px grow bg-border" style={lineStyle} />
        <span style={textStyle}>{item.name} mode</span>
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
        <div className="rounded-xl rounded-br-sm border border-border bg-raised px-3.5 py-2.5 [overflow-wrap:break-word]">
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
              aria-label="Rejouer"
              title="Rejouer"
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
    return <AssistantItemView item={item} model={model} running={running} showHeader={showHeader} />;
  }

  if (item.kind === 'error') {
    return <div className="rounded-md border border-border bg-raised px-3 py-2 text-sm text-err">{item.message}</div>;
  }

  if (item.kind === 'note') {
    if (item.detail !== undefined) {
      return (
        <details>
          <summary className="cursor-pointer text-center text-xs text-muted">{item.text}</summary>
          <div className="mt-2 rounded-md border border-border bg-raised px-3 py-2 text-left text-sm">
            <TextSegment content={item.detail} caret={false} />
          </div>
        </details>
      );
    }
    return <div className="text-center text-xs text-muted">{item.text}</div>;
  }

  if (item.kind === 'workflow-node') {
    return <NodeDivider item={item} running={running} />;
  }

  if (item.kind === 'workflow-end') {
    return <RunEndCard item={item} />;
  }
}
