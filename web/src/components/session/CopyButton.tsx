import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { CheckIcon, CopyIcon } from '../Icons';
import { Button } from '../ui/Button';

export function CopyButton({ text }: { text: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timer.current != null) {
        window.clearTimeout(timer.current);
      }
    };
  }, []);

  const handleClick = () => {
    navigator.clipboard.writeText(text).catch(() => {
      // The clipboard can be unavailable; the button still gives feedback.
    });
    setCopied(true);
    if (timer.current != null) {
      window.clearTimeout(timer.current);
    }
    timer.current = window.setTimeout(() => {
      setCopied(false);
    }, 1500);
  };

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      iconOnly
      aria-label={t('copyButton')}
      title={t('copyButton')}
      onClick={handleClick}
      className="shrink-0"
    >
      {copied ? <CheckIcon className="h-3.5 w-3.5 text-ok" /> : <CopyIcon className="h-3.5 w-3.5" />}
    </Button>
  );
}
