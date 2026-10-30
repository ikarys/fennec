/**
 * The add-idea field, opened on demand above the tasks; stays open to chain ideas.
 * Files and images can be pinned to the idea; they go to the agent on framing and build.
 */
import { useRef, useState } from 'react';
import { Button } from '../../ui/Button';
import { TextInput } from '../../ui/Field';
import { CloseIcon, FileIcon, PaperclipIcon, PlusIcon } from '../../Icons';
import { addIdea, type IdeaBoard } from '../../../lib/ideas';
import { readFiles, type FileAttachmentInput } from '../../../lib/attachments';

export function AddIdeaField({
  projectId,
  run,
  onClose,
}: {
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  onClose: () => void;
}) {
  const [text, setText] = useState('');
  const [attachments, setAttachments] = useState<FileAttachmentInput[]>([]);
  const [attachError, setAttachError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleAttach = async (files: File[]) => {
    const { attachments: read, error } = await readFiles(files, attachments.length);
    if (error) {
      setAttachError(error);
      return;
    }
    setAttachError(null);
    setAttachments((current) => [...current, ...read]);
  };

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (files && files.length > 0) void handleAttach([...files]);
    event.target.value = '';
  };

  const handleAdd = async () => {
    const value = text.trim();
    if (!value || busy) return;
    setBusy(true);
    try {
      const ok = await run(() => addIdea(projectId, value, attachments));
      if (ok) {
        setText('');
        setAttachments([]);
        setAttachError(null);
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="relative">
      {attachments.length > 0 ? (
        <div className="mb-1.5 flex flex-wrap gap-1.5">
          {attachments.map((attachment, i) =>
            attachment.mimeType.startsWith('image/') ? (
              <div key={i} className="relative h-10 w-10">
                <img
                  src={attachment.data}
                  alt={attachment.filename}
                  className="h-10 w-10 rounded-md border border-border object-cover"
                />
                <button
                  type="button"
                  aria-label={`Retirer ${attachment.filename}`}
                  onClick={() => setAttachments((current) => current.filter((_, j) => j !== i))}
                  className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full border border-border-strong bg-bg text-muted hover:text-text"
                >
                  <CloseIcon className="h-2.5 w-2.5" />
                </button>
              </div>
            ) : (
              <div
                key={i}
                className="flex items-center gap-1 rounded-md border border-border bg-bg px-2 py-1 text-xs text-text"
              >
                <FileIcon className="h-3.5 w-3.5 text-muted" />
                <span className="max-w-[160px] truncate" title={attachment.filename}>
                  {attachment.filename}
                </span>
                <button
                  type="button"
                  aria-label={`Retirer ${attachment.filename}`}
                  onClick={() => setAttachments((current) => current.filter((_, j) => j !== i))}
                  className="text-muted hover:text-text"
                >
                  <CloseIcon className="h-3 w-3" />
                </button>
              </div>
            ),
          )}
        </div>
      ) : null}

      <TextInput
        autoFocus
        className="w-full pr-16"
        placeholder="Une idée à garder pour plus tard… (Échap pour fermer)"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          if (!text.trim() && attachments.length === 0) onClose();
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void handleAdd();
          } else if (e.key === 'Escape') {
            onClose();
          }
        }}
      />
      <div className="absolute right-1 top-1/2 flex -translate-y-1/2 items-center gap-0.5">
        <Button
          variant="ghost"
          iconOnly
          title="Joindre un fichier ou une image"
          aria-label="Joindre un fichier ou une image"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => fileInputRef.current?.click()}
        >
          <PaperclipIcon className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          iconOnly
          title="Ajouter l'idée au backlog (Entrée)"
          aria-label="Ajouter l'idée au backlog"
          disabled={!text.trim() || busy}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => void handleAdd()}
        >
          <PlusIcon className="h-4 w-4" />
        </Button>
        <input
          ref={fileInputRef}
          type="file"
          multiple
          className="hidden"
          onChange={handleFileChange}
        />
      </div>
      {attachError ? <p className="mt-1 text-xs text-err">{attachError}</p> : null}
    </div>
  );
}
