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

const markdownComponents = {
  pre: ({ children }: { children?: React.ReactNode }) => (
    <pre className="my-2 overflow-x-auto rounded-md bg-code p-3 font-mono text-xs text-muted">{children}</pre>
  ),
  code: ({ className, children }: { className?: string; children?: React.ReactNode }) => {
    const isBlock = className?.includes('language-');
    if (isBlock) {
      return <code className="font-mono text-xs">{children}</code>;
    }
    return (
      <code className="rounded bg-raised px-1 py-0.5 font-mono text-[12.5px] text-text">{children}</code>
    );
  },
  p: ({ children }: { children?: React.ReactNode }) => <p className="my-2 leading-relaxed">{children}</p>,
  // Tailwind's preflight removes list markers; plans are lists of lists.
  ul: ({ children }: { children?: React.ReactNode }) => <ul className="my-2 list-disc pl-5">{children}</ul>,
  ol: ({ children }: { children?: React.ReactNode }) => <ol className="my-2 list-decimal pl-5">{children}</ol>,
  li: ({ children }: { children?: React.ReactNode }) => <li className="my-0.5 leading-relaxed">{children}</li>,
  a: ({ children, href }: { children?: React.ReactNode; href?: string }) => (
    <a href={href} className="text-accent-strong underline" target="_blank" rel="noreferrer">
      {children}
    </a>
  ),
  table: ({ children }: { children?: React.ReactNode }) => (
    // Wide tables scroll inside the thread instead of being cut by it.
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-sm">{children}</table>
    </div>
  ),
  th: ({ children }: { children?: React.ReactNode }) => (
    <th className="border border-border bg-raised px-2 py-1 text-left">{children}</th>
  ),
  td: ({ children }: { children?: React.ReactNode }) => <td className="border border-border px-2 py-1">{children}</td>
};

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
        className="flex min-h-8 items-center gap-2 self-start rounded-lg border border-dashed border-border bg-transparent px-2.5 text-[13px] text-muted"
      >
        <BulbIcon className="h-3.5 w-3.5" />
        Réflexion · {open ? 'masquer' : 'afficher'}
      </button>
      {open && (
        <div className="border-l-2 border-border pl-3.5 text-[13px] whitespace-pre-line text-muted">{content}</div>
      )}
    </div>
  );
});

const REMARK_PLUGINS = [remarkGfm];

const TextSegment = memo(function TextSegment({ content, caret }: { content: string; caret: boolean }) {
  // Parsed only when settled: a half-written contract would flicker between the
  // two renderings while the message streams.
  const contract = useMemo(() => (caret ? null : splitContract(content)), [content, caret]);
  return (
    <div className="min-w-0 text-[14px] leading-relaxed [overflow-wrap:break-word]">
      {contract ? (
        <>
          {contract.before && (
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {contract.before}
            </ReactMarkdown>
          )}
          {contract.contract.kind === 'plan' ? <PlanCard plan={contract.contract} /> : <ReportCard report={contract.contract} />}
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
          <span className="rounded border border-border px-1.5 text-[11px]">produit par le modèle</span>
        </div>
      )}
      {item.segments.map((segment) => {
        if (segment.kind === 'thinking') {
          return <ThinkingSegment key={segment.id} content={segment.content} />;
        }
        if (segment.kind === 'text') {
          return <TextSegment key={segment.id} content={segment.content} caret={streaming && segment.id === lastTextId} />;
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
      <div className="flex items-center gap-2 text-[11px] text-muted">
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
                <img key={a.id} src={a.data} alt={a.filename} className="max-h-48 rounded-lg border border-border" />
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
}
