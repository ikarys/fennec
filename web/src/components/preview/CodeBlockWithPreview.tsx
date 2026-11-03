import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useT } from '../../i18n';
import { markdownComponents } from '../session/markdownComponents';
import type { PreviewKind } from '../../lib/preview';

const REMARK_PLUGINS = [remarkGfm];
/** How long the source must stay still before the iframe reloads it. */
const SETTLE_MS = 400;

/** The source once it stopped changing: a streamed block would reload the iframe at every chunk. */
function useSettled(source: string): string {
  const [settled, setSettled] = useState(source);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(source), SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, [source]);
  return settled;
}

export function CodeBlockWithPreview({ kind, source }: { kind: PreviewKind; source: string }) {
  const t = useT();
  // An HTML block is mostly a page to look at; its source stays one click away.
  const [showPreview, setShowPreview] = useState(kind === 'html');
  const settledSource = useSettled(source);

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
            {t('previewCode')}
          </button>
          <button
            type="button"
            aria-pressed={showPreview}
            onClick={() => setShowPreview(true)}
            className={`rounded px-1.5 py-0.5 ${
              showPreview ? 'bg-accent-soft text-accent-strong' : 'text-muted hover:text-text'
            }`}
          >
            {t('preview')}
          </button>
        </div>
      </div>
      {showPreview ? (
        kind === 'html' ? (
          <iframe
            sandbox="allow-scripts"
            srcDoc={settledSource}
            title={t('previewHtmlTitle')}
            className="block h-[60vh] w-full bg-panel"
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
