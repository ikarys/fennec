import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import type { Agent, AttachmentInput, SlashCommand } from '../../lib/api';
import { formatTokens } from '../../lib/format';
import { IMAGE_TYPES } from '../../lib/attachments';
import { CloseIcon, PaperclipIcon, SendIcon, StopIcon } from '../Icons';
import { matchCommands, SlashMenu } from './SlashMenu';
import { ModePicker } from './ModePicker';

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
  commands?: SlashCommand[];
  agents?: Agent[];
  mode?: string | null;
  onModeChange?: (id: string) => void;
  attachments?: AttachmentInput[];
  onAttach?: (files: File[]) => void;
  onRemoveAttachment?: (index: number) => void;
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
  placeholder,
  commands,
  agents,
  mode,
  onModeChange,
  attachments,
  onAttach,
  onRemoveAttachment
}: ComposerProps) {
  const [selected, setSelected] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const matches = matchCommands(text, commands ?? []);
  const menuOpen = matches.length > 0 && !dismissed;
  const matchKey = matches.map((command) => command.name).join('\n');

  useEffect(() => {
    setDismissed(false);
  }, [text]);

  useEffect(() => {
    setSelected(0);
  }, [matchKey]);

  function selectCommand(index: number) {
    const command = matches[index];
    if (command) {
      onTextChange(`/${command.name} `);
    }
  }

  const canSend = !running && (text.trim().length > 0 || (attachments?.length ?? 0) > 0);

  function handleFileChange(event: React.ChangeEvent<HTMLInputElement>) {
    const files = event.target.files;
    if (files && files.length > 0) {
      onAttach?.([...files]);
    }
    event.target.value = '';
  }

  function handlePaste(event: React.ClipboardEvent<HTMLTextAreaElement>) {
    if (!onAttach) return;
    const files = event.clipboardData?.files;
    if (files && files.length > 0) {
      event.preventDefault();
      onAttach([...files]);
    }
  }

  function handleDrop(event: React.DragEvent<HTMLDivElement>) {
    if (!onAttach) return;
    event.preventDefault();
    const files = event.dataTransfer?.files;
    if (files && files.length > 0) {
      onAttach([...files]);
    }
  }

  function handleDragOver(event: React.DragEvent<HTMLDivElement>) {
    if (!onAttach) return;
    event.preventDefault();
  }

  return (
    <div className="px-6 py-3 pb-5">
      <div
        className="mx-auto max-w-[820px] rounded-[14px] border border-border-strong bg-panel focus-within:border-accent"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        {menuOpen && <SlashMenu commands={matches} selected={selected} onSelect={selectCommand} />}
        <label htmlFor="prompt" className="absolute h-px w-px overflow-hidden clip-[rect(0_0_0_0)]">
          Message
        </label>
        {attachments && attachments.length > 0 && (
          <div className="flex flex-wrap gap-2 px-3.5 pt-3">
            {attachments.map((a, i) => (
              <div key={i} className="relative h-14 w-14">
                <img src={a.data} alt={a.filename} className="h-14 w-14 rounded-lg border border-border object-cover" />
                <button
                  type="button"
                  aria-label={`Retirer ${a.filename}`}
                  onClick={() => onRemoveAttachment?.(i)}
                  className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full border border-border bg-raised text-muted"
                >
                  <CloseIcon className="h-2.5 w-2.5" />
                </button>
              </div>
            ))}
          </div>
        )}
        <textarea
          id="prompt"
          ref={textareaRef}
          rows={2}
          value={text}
          onChange={(event) => onTextChange(event.target.value)}
          onPaste={handlePaste}
          onKeyDown={(event) => {
            if (menuOpen) {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setSelected((current) => Math.min(current + 1, matches.length - 1));
                return;
              }
              if (event.key === 'ArrowUp') {
                event.preventDefault();
                setSelected((current) => Math.max(current - 1, 0));
                return;
              }
              if (event.key === 'Enter' || event.key === 'Tab') {
                event.preventDefault();
                selectCommand(selected);
                return;
              }
              if (event.key === 'Escape') {
                event.preventDefault();
                setDismissed(true);
                return;
              }
            }
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault();
              onSend();
            }
          }}
          placeholder={placeholder ?? "Demande quelque chose… (@ pour un fichier, / pour une commande)"}
          className="w-full resize-none bg-transparent p-3.5 pt-3 text-sm outline-none"
        />
        <div className="flex flex-wrap items-center gap-2 px-2.5 pb-2 pt-1.5">
          {onAttach && (
            <>
              <button
                type="button"
                aria-label="Joindre une image"
                onClick={() => fileInputRef.current?.click()}
                className="flex h-8 w-8 items-center justify-center rounded-lg border border-border bg-raised text-muted hover:text-text"
              >
                <PaperclipIcon className="h-3.5 w-3.5" />
              </button>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                accept={IMAGE_TYPES.join(',')}
                className="hidden"
                onChange={handleFileChange}
              />
            </>
          )}
          {agents && onModeChange && <ModePicker agents={agents} mode={mode ?? null} onChange={onModeChange} />}
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
            disabled={!canSend}
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
