import { useState } from 'react';
import type { SeenImage } from '../../lib/api';
import { panelBlock } from '../../lib/panelBlocks';
import { CloseIcon, EyeIcon, LocateIcon } from '../Icons';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Modal } from '../ui/Modal';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';

export function VisionSection({ images }: { images: SeenImage[] }) {
  const t = useT();
  const [selected, setSelected] = useState<SeenImage | null>(null);

  const closeDialog = () => {
    setSelected(null);
  };

  const jumpToThread = () => {
    const anchorId = selected?.anchorId;
    closeDialog();
    if (anchorId) {
      document.getElementById(anchorId)?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  return (
    <Panel
      id="vision"
      title={t(panelBlock('vision').titleKey)}
      icon={<EyeIcon className="h-3.5 w-3.5" />}
      summary={images.length > 0 ? images.length : undefined}
    >
      <p className="m-0 text-xs text-muted">{t('visionImagesHint')}</p>
      {images.length === 0 ? (
        <div className="text-xs text-muted">{t('visionNoImages')}</div>
      ) : (
        <div className="grid grid-cols-3 gap-1.5">
          {images.map((image, index) => (
            <button
              key={`${image.anchorId}-${index}`}
              type="button"
              aria-label={t('visionEnlarge', { label: image.label })}
              title={image.inContext ? image.label : t('outOfContext')}
              onClick={() => setSelected(image)}
              className={`block w-full overflow-hidden rounded-md ${image.inContext ? '' : 'opacity-40'}`}
            >
              <img src={image.src} alt={image.label} className="aspect-square w-full rounded-md border border-border object-cover" />
            </button>
          ))}
        </div>
      )}
      <Modal
        open={selected !== null}
        onClose={closeDialog}
        label={selected?.label ?? t('panelVision')}
        width="w-auto max-w-[min(90vw,1100px)]"
      >
        {selected && (
          <Card className="flex flex-col gap-3 p-4">
            <img src={selected.src} alt={selected.label} className="max-h-[75vh] max-w-full object-contain" />
            <div className="flex flex-col gap-1">
              <div className="font-mono text-xs text-muted">{selected.label}</div>
              {!selected.inContext && <div className="text-xs text-muted">{t('outOfContext')}</div>}
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="secondary"
                size="sm"
                iconOnly
                onClick={jumpToThread}
                title={t('visionViewInThread')}
                aria-label={t('visionViewInThread')}
              >
                <LocateIcon className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                onClick={closeDialog}
                title={t('close')}
                aria-label={t('close')}
              >
                <CloseIcon className="h-4 w-4" />
              </Button>
            </div>
          </Card>
        )}
      </Modal>
    </Panel>
  );
}
