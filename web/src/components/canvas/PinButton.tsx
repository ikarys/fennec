import { useState } from 'react';
import { Button } from '../ui/Button';
import { PinIcon, SpinnerIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { useCanvasOpener } from './CanvasContext';
import type { CanvasArtifact } from '../../lib/api/canvas';

export function PinButton({ pin }: { pin: () => Promise<CanvasArtifact> }) {
  const t = useT();
  const opener = useCanvasOpener();
  const [pinning, setPinning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!opener) return null;

  const handleClick = async () => {
    setPinning(true);
    setError(null);
    try {
      const artifact = await pin();
      opener(artifact.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setPinning(false);
    }
  };

  return (
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      title={error ?? t('canvasPin')}
      aria-label={t('canvasPin')}
      disabled={pinning}
      onClick={() => void handleClick()}
    >
      {pinning ? <SpinnerIcon className="h-4 w-4" /> : <PinIcon className="h-4 w-4" />}
    </Button>
  );
}
