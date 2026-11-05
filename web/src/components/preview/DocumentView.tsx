import type { ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useT } from '../../i18n';
import { useFetchedText } from '../../hooks/useFetchedText';
import { markdownComponents } from '../session/markdownComponents';
import type { PreviewKind } from '../../lib/preview';
import { JsonView } from './JsonView';
import { MermaidDiagram } from './MermaidDiagram';

const REMARK_PLUGINS = [remarkGfm];

interface DocumentViewProps {
  kind: PreviewKind;
  url: string;
  title: string;
  reloadKey?: unknown;
  overlay?: ReactNode;
  onTextMouseUp?: () => void;
}

export function DocumentView({ kind, url, title, reloadKey, overlay, onTextMouseUp }: DocumentViewProps) {
  const t = useT();
  const textUrl = kind === 'markdown' || kind === 'json' || kind === 'mermaid' ? url : undefined;
  const { text, loading, error } = useFetchedText(textUrl, reloadKey);

  if (kind === 'html') {
    return (
      <div className="relative min-h-0 grow">
        <iframe
          key={String(reloadKey)}
          sandbox="allow-scripts"
          src={url}
          title={title}
          className="h-full w-full bg-panel"
        />
        {overlay}
      </div>
    );
  }

  if (kind === 'pdf') {
    return (
      <div className="relative min-h-0 grow">
        {/* Browsers' PDF viewers refuse a sandboxed frame; the server serves a PDF unsandboxed only when its bytes are a real PDF, and the viewer runs it in its own origin. */}
        <iframe key={String(reloadKey)} src={url} title={title} className="h-full w-full bg-panel" />
        {overlay}
      </div>
    );
  }

  if (kind === 'markdown' || kind === 'json') {
    return (
      <div className="min-h-0 grow overflow-y-auto p-6">
        <div className="relative mx-auto max-w-3xl" onMouseUp={onTextMouseUp}>
          {error ? (
            <div className="text-sm text-err">{error}</div>
          ) : loading || text == null ? (
            <div className="text-sm text-muted">{t('loading')}</div>
          ) : kind === 'markdown' ? (
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {text}
            </ReactMarkdown>
          ) : (
            <JsonView source={text} />
          )}
          {overlay}
        </div>
      </div>
    );
  }

  if (kind === 'mermaid') {
    return (
      <div className="flex min-h-0 grow items-center justify-center overflow-auto bg-panel p-6">
        <div className="relative">
          {error ? (
            <div className="text-sm text-err">{error}</div>
          ) : loading || text == null ? (
            <div className="text-sm text-muted">{t('loading')}</div>
          ) : (
            <MermaidDiagram source={text} />
          )}
          {overlay}
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 grow items-center justify-center overflow-auto bg-panel p-6">
      <div className="relative">
        <img key={String(reloadKey)} src={url} alt={title} className="max-w-full" />
        {overlay}
      </div>
    </div>
  );
}
