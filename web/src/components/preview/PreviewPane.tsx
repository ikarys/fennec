import { useEffect, useState } from 'react';
import { Button, buttonClass } from '../ui/Button';
import { CloseIcon, RefreshIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { previewUrl } from '../../lib/preview';
import { pinCanvasFile } from '../../lib/api/canvas';
import { PinButton } from '../canvas/PinButton';
import { DocumentView } from './DocumentView';
import type { PreviewTarget } from './PreviewContext';

interface PreviewPaneProps {
  sessionId: string;
  target: PreviewTarget;
  onClose: () => void;
}

export function PreviewPane({ sessionId, target, onClose }: PreviewPaneProps) {
  const t = useT();
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onClose]);

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
      <DocumentView
        kind={target.kind}
        url={previewUrl(sessionId, target.path)}
        title={t('preview')}
        reloadKey={reloadKey}
      />
    </div>
  );
}
