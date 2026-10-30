/**
 * The app's one modal. The <dialog> element, the shared backdrop (the bg-scrim
 * token) and the close behaviour (Escape, click on the backdrop, no close while
 * a request is in flight) live here so every screen is the same; each surface
 * keeps its own Card content.
 */
import { useEffect, useRef, type ReactNode } from 'react';

export function Modal({
  open,
  onClose,
  busy = false,
  label,
  width = 'w-[min(92vw,560px)]',
  children,
}: {
  open: boolean;
  onClose: () => void;
  /** A request is in flight: Escape and the backdrop do not close it. */
  busy?: boolean;
  /** The dialog's accessible name. */
  label?: string;
  /** The dialog's width; a lightbox passes a wide one. */
  width?: string;
  children: ReactNode;
}) {
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
        if (busy) event.preventDefault();
        else onClose();
      }}
      onClick={(event) => {
        if (event.target === dialogRef.current && !busy) onClose();
      }}
      className={`m-auto text-text backdrop:bg-scrim ${width}`}
    >
      {children}
    </dialog>
  );
}
