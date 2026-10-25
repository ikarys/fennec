import { useEffect, useRef, useState } from 'react';
import { CheckIcon, CopyIcon } from '../Icons';

export function CopyButton({ text }: { text: string }) {
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
    <button
      type="button"
      aria-label="Copier"
      title="Copier"
      onClick={handleClick}
      className="rounded-md p-1 text-muted transition-colors hover:text-text"
    >
      {copied ? <CheckIcon className="h-3.5 w-3.5 text-ok" /> : <CopyIcon className="h-3.5 w-3.5" />}
    </button>
  );
}
