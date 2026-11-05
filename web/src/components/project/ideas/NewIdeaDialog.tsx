/**
 * The add-idea dialog: a multilinear text and images pasted straight from the
 * clipboard (Ctrl+V). Files and images go to the agent on framing and build.
 */
import { useRef, useState } from 'react';
import { Button } from '../../ui/Button';
import { Modal } from '../../ui/Modal';
import { TextArea } from '../../ui/Field';
import { CloseIcon, FileIcon, PaperclipIcon } from '../../Icons';
import { addIdea, type IdeaBoard } from '../../../lib/ideas';
import { readFiles, type FileAttachmentInput } from '../../../lib/attachments';
import { useT, useLang } from '../../../i18n';

export function NewIdeaDialog({
  projectId,
  open,
  run,
  onClose,
}: {
  projectId: string;
  open: boolean;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  onClose: () => void;
}) {
  const lang = useLang();
  const t = useT();
  const [text, setText] = useState('');
  const [attachments, setAttachments] = useState<FileAttachmentInput[]>([]);
  const [attachError, setAttachError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const handleAttach = async (files: File[]) => {
    const { attachments: read, error } = await readFiles(lang, files, attachments.length);
    if (error) {
      setAttachError(error);
      return;
    }
    setAttachError(null);
    setAttachments((current) => [...current, ...read]);
  };

  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files ? [...event.target.files] : [];
    if (files.length > 0) {
      void handleAttach(files);
    }
    if (event.target) {
      event.target.value = '';
    }
  };

  const handlePaste = (event: React.ClipboardEvent) => {
    const files = [...(event.clipboardData?.files ?? [])].filter((f) =>
      f.type.startsWith('image/'),
    );
    if (files.length > 0) {
      event.preventDefault();
      void handleAttach(files);
    }
  };

  const reset = () => {
    setText('');
    setAttachments([]);
    setAttachError(null);
  };

  const handleAdd = async () => {
    const value = text.trim();
    if (!value || busy) return;
    setBusy(true);
    try {
      const ok = await run(() => addIdea(projectId, value, attachments));
      if (ok) {
        reset();
        onClose();
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} busy={busy} label={t('newIdea')}>
      <div className="flex flex-col gap-3 p-4">
        <div className="flex items-center justify-between">
          <h2 className="m-0 text-sm font-medium">{t('newIdea')}</h2>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('close')}
            aria-label={t('close')}
            onClick={onClose}
          >
            <CloseIcon className="h-4 w-4" />
          </Button>
        </div>

        <TextArea
          autoFocus
          rows={4}
          placeholder={t('newIdeaPlaceholder')}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onPaste={handlePaste}
        />

        {attachments.length > 0 ? (
          <div className="flex flex-wrap gap-1.5">
            {attachments.map((attachment, i) =>
              attachment.mimeType.startsWith('image/') ? (
                <div key={i} className="relative h-14 w-14">
                  <img
                    src={attachment.data}
                    alt={attachment.filename}
                    className="h-14 w-14 rounded-md border border-border object-cover"
                  />
                  <button
                    type="button"
                    aria-label={t('removeAttachment', { name: attachment.filename })}
                    onClick={() =>
                      setAttachments((current) => current.filter((_, j) => j !== i))
                    }
                    className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full border border-border-strong bg-bg text-muted hover:text-err"
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
                    aria-label={t('removeAttachment', { name: attachment.filename })}
                    onClick={() =>
                      setAttachments((current) => current.filter((_, j) => j !== i))
                    }
                    className="text-muted hover:text-err"
                  >
                    <CloseIcon className="h-3 w-3" />
                  </button>
                </div>
              ),
            )}
          </div>
        ) : null}

        <p className="m-0 text-xs text-faint">{t('pasteImageHint')}</p>
        {attachError ? <p className="m-0 text-xs text-err">{attachError}</p> : null}

        <div className="flex items-center justify-between border-t border-border pt-3">
          <div className="flex items-center gap-2">
            <input
              ref={fileInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={handleFileChange}
            />
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              title={t('attachFileTitle')}
              aria-label={t('attachFileTitle')}
              onClick={() => fileInputRef.current?.click()}
            >
              <PaperclipIcon className="h-4 w-4" />
            </Button>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" onClick={onClose}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              disabled={!text.trim() || busy}
              onClick={() => void handleAdd()}
            >
              {t('addIdeaBtn')}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
