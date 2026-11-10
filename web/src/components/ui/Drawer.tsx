/**
 * A panel docked to the right edge: the same native <dialog> as Modal (focus
 * trap, Escape, click on the backdrop to close), with a header for the title
 * and its actions.
 */
import { useEffect, useRef, type ReactNode } from 'react';
import { Button } from './Button';
import { CloseIcon } from './Icons';
import { useT } from '../../i18n';

export function Drawer({
  open,
  onClose,
  label,
  title,
  actions,
  children,
}: {
  open: boolean;
  onClose: () => void;
  /** The drawer's accessible name. */
  label: string;
  title: ReactNode;
  /** Shown in the header, right of the title. */
  actions?: ReactNode;
  children: ReactNode;
}) {
  const t = useT();
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    if (open && !el.open) el.showModal();
    if (!open && el.open) el.close();
  }, [open]);

  return (
    <dialog
      ref={dialogRef}
      aria-label={label}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => {
        if (event.target === dialogRef.current) onClose();
      }}
      className="ml-auto mr-0 my-0 h-dvh max-h-dvh w-[min(720px,100%)] max-w-none flex-col border-l border-border bg-panel p-0 text-text backdrop:bg-scrim open:flex max-[900px]:w-full"
    >
      <div className="flex items-center gap-2 border-b border-border bg-accent-soft/50 px-4 py-2.5">
        <div className="min-w-0 grow truncate font-semibold">{title}</div>
        {actions}
        <Button variant="ghost" size="sm" iconOnly title={t('close')} aria-label={t('close')} onClick={onClose}>
          <CloseIcon className="h-4 w-4" />
        </Button>
      </div>
      <div className="min-h-0 grow overflow-y-auto p-4">{children}</div>
    </dialog>
  );
}
