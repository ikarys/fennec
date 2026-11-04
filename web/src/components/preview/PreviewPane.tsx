import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Button, buttonClass } from '../ui/Button';
import { CloseIcon, RefreshIcon } from '../Icons';
import { markdownComponents } from '../session/markdownComponents';
import { useT } from '../../i18n';
import { previewUrl } from '../../lib/preview';
import { pinCanvasFile } from '../../lib/api/canvas';
import { PinButton } from '../canvas/PinButton';
import type { PreviewTarget } from './PreviewContext';

const REMARK_PLUGINS = [remarkGfm];

interface PreviewPaneProps {
  sessionId: string;
  target: PreviewTarget;
  onClose: () => void;
}

export function PreviewPane({ sessionId, target, onClose }: PreviewPaneProps) {
  const t = useT();
  const [reloadKey, setReloadKey] = useState(0);
  const [markdown, setMarkdown] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onClose]);

  useEffect(() => {
    if (target.kind !== 'markdown') return;
    let active = true;
    setLoading(true);
    setError(null);
    setMarkdown(null);
    const url = previewUrl(sessionId, target.path);
    fetch(url)
      .then(async (response) => {
        if (!response.ok) {
          let message = response.statusText;
          try {
            const body = (await response.json()) as { error?: string };
            if (body.error) message = body.error;
          } catch {
            // Keep the status text when the error body is not JSON.
          }
          throw new Error(message);
        }
        return response.text();
      })
      .then((text) => {
        if (active) {
          setMarkdown(text);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setError(err instanceof Error ? err.message : t('previewLoadError'));
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [sessionId, target.path, target.kind, reloadKey]);

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-[min(960px,92vw)] flex-col border-l border-border bg-bg shadow-xl">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <span className="min-w-0 grow truncate font-mono text-xs text-muted">{target.path}</span>
        <PinButton
          pin={async () => {
            const artifact = await pinCanvasFile(sessionId, target.path);
            window.setTimeout(onClose, 0);
            return artifact;
          }}
        />
        <Button
          variant="ghost"
          iconOnly
          title={t('previewReload')}
          aria-label={t('previewReload')}
          onClick={() => setReloadKey((value) => value + 1)}
        >
          <RefreshIcon className="h-4 w-4" />
        </Button>
        <a
          href={previewUrl(sessionId, target.path)}
          target="_blank"
          rel="noreferrer"
          className={buttonClass()}
        >
          {t('previewOpenTab')}
        </a>
        <Button variant="ghost" iconOnly title={t('close')} aria-label={t('close')} onClick={onClose}>
          <CloseIcon className="h-4 w-4" />
        </Button>
      </header>
      {target.kind === 'html' ? (
        <iframe
          key={reloadKey}
          sandbox="allow-scripts"
          src={previewUrl(sessionId, target.path)}
          title={t('preview')}
          className="w-full grow bg-panel"
        />
      ) : (
        <div className="min-h-0 grow overflow-y-auto p-6">
          {loading ? (
            <div className="text-sm text-muted">{t('loading')}</div>
          ) : error ? (
            <div className="text-sm text-err">{error}</div>
          ) : markdown != null ? (
            <div className="mx-auto max-w-3xl">
              <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                {markdown}
              </ReactMarkdown>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}
