import { memo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { AssistantItem, Segment, ThreadItem } from '../../lib/fold';
import { BulbIcon } from '../Icons';
import { MessageStatsLine } from './MessageStats';
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
  const [open, setOpen] = useState(false);
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
  return (
    <div className="min-w-0 text-[14px] leading-relaxed [overflow-wrap:break-word]">
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
        {content}
      </ReactMarkdown>
      {caret && <span className="caret ml-0.5 inline-block h-4 w-2 bg-accent align-text-bottom" />}
    </div>
  );
});

function AssistantItemView({ item, model, running, showHeader }: { item: AssistantItem; model: string; running: boolean; showHeader: boolean }) {
  const lastTextId = [...item.segments].reverse().find((segment): segment is Extract<Segment, { kind: 'text' }> => segment.kind === 'text')?.id;
  const streaming = running && !item.done;

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
      <MessageStatsLine item={item} streaming={streaming} />
    </div>
  );
}

export function ThreadItemView({ item, model, running, showHeader }: { item: ThreadItem; model: string; running: boolean; showHeader: boolean }) {
  if (item.kind === 'user') {
    if (item.isSystemGenerated) {
      return <div className="text-center text-xs text-muted">{item.content}</div>;
    }
    return (
      <div className="max-w-[85%] self-end rounded-xl rounded-br-sm border border-border bg-raised px-3.5 py-2.5 whitespace-pre-wrap [overflow-wrap:break-word]">
        {item.content}
      </div>
    );
  }

  if (item.kind === 'assistant') {
    return <AssistantItemView item={item} model={model} running={running} showHeader={showHeader} />;
  }

  if (item.kind === 'error') {
    return <div className="rounded-md border border-border bg-raised px-3 py-2 text-sm text-err">{item.message}</div>;
  }

  return <div className="text-center text-xs text-muted">{item.text}</div>;
}

