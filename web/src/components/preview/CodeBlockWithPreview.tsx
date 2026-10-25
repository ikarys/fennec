import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { markdownComponents } from '../session/markdownComponents';
import type { PreviewKind } from '../../lib/preview';

const REMARK_PLUGINS = [remarkGfm];

export function CodeBlockWithPreview({ kind, source }: { kind: PreviewKind; source: string }) {
  const [showPreview, setShowPreview] = useState(false);

  return (
    <div className="my-2 overflow-hidden rounded-md border border-border">
      <div className="flex items-center justify-between gap-2 border-b border-border bg-raised px-2.5 py-1.5 text-xs">
        <span className="font-mono text-muted">{kind}</span>
        <div className="flex gap-1">
          <button
            type="button"
            aria-pressed={!showPreview}
            onClick={() => setShowPreview(false)}
            className={`rounded px-1.5 py-0.5 ${
              !showPreview ? 'bg-accent-soft text-accent-strong' : 'text-muted hover:text-text'
            }`}
          >
            Code
          </button>
          <button
            type="button"
            aria-pressed={showPreview}
            onClick={() => setShowPreview(true)}
            className={`rounded px-1.5 py-0.5 ${
              showPreview ? 'bg-accent-soft text-accent-strong' : 'text-muted hover:text-text'
            }`}
          >
            Aperçu
          </button>
        </div>
      </div>
      {showPreview ? (
        kind === 'html' ? (
          <iframe
            sandbox="allow-scripts"
            srcDoc={source}
            title="Aperçu HTML"
            className="block h-[60vh] w-full bg-white"
          />
        ) : (
          <div className="px-4 py-2">
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {source}
            </ReactMarkdown>
          </div>
        )
      ) : (
        <pre className="overflow-x-auto bg-code p-3 font-mono text-xs text-muted">{source}</pre>
      )}
    </div>
  );
}
