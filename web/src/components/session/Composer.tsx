import type { RefObject } from 'react';
import { formatTokens } from '../../lib/format';
import { SendIcon, StopIcon } from '../Icons';

interface ComposerProps {
  text: string;
  onTextChange: (text: string) => void;
  onSend: () => void;
  onStop: () => void;
  running: boolean;
  model: string | undefined;
  currentTokens: number;
  maxTokens: number;
  error: string | null;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  placeholder?: string;
}

export function Composer({
  text,
  onTextChange,
  onSend,
  onStop,
  running,
  model,
  currentTokens,
  maxTokens,
  error,
  textareaRef,
  placeholder
}: ComposerProps) {
  return (
    <div className="px-6 py-3 pb-5">
      <div className="mx-auto max-w-[820px] rounded-[14px] border border-border-strong bg-panel focus-within:border-accent">
        <label htmlFor="prompt" className="absolute h-px w-px overflow-hidden clip-[rect(0_0_0_0)]">
          Message
        </label>
        <textarea
          id="prompt"
          ref={textareaRef}
          rows={2}
          value={text}
          onChange={(event) => onTextChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault();
              onSend();
            }
          }}
          placeholder={placeholder ?? "Demande quelque chose… (@ pour un fichier, / pour une commande)"}
          className="w-full resize-none bg-transparent p-3.5 pt-3 text-sm outline-none"
        />
        <div className="flex flex-wrap items-center gap-2 px-2.5 pb-2 pt-1.5">
          <button
            type="button"
            disabled
            className="flex min-h-8 items-center gap-1.5 rounded-lg border border-border bg-raised px-2.5 text-xs text-text"
          >
            <span className="h-1.5 w-1.5 rounded-full bg-ok" />
            {model ?? '…'} · local
          </button>
          <span className="text-xs text-muted">
            {formatTokens(currentTokens)} / {formatTokens(maxTokens)} tokens
          </span>
          <span className="grow" />
          {running && (
            <button
              type="button"
              aria-label="Arrêter"
              onClick={onStop}
              className="flex h-9 w-9 items-center justify-center rounded-[10px] border border-border bg-transparent text-text"
            >
              <StopIcon className="h-3.5 w-3.5" />
            </button>
          )}
          <button
            type="button"
            aria-label="Envoyer"
            onClick={onSend}
            disabled={running || !text.trim()}
            className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-accent text-on-accent disabled:opacity-60"
          >
            <SendIcon className="h-4 w-4" />
          </button>
        </div>
        {error && <div className="px-3 pb-2 text-xs text-err">{error}</div>}
      </div>
    </div>
  );
}
