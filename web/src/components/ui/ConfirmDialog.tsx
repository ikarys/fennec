/**
 * The app's one confirmation: a question and two buttons, on the shared Modal.
 * Every in-app "are you sure" looks the same, and none of them is a browser
 * confirm().
 */
import type { ReactNode } from 'react';
import { useT } from '../../i18n';
import { Button, type ButtonVariant } from './Button';
import { Card } from './Card';
import { Modal } from './Modal';

export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  variant = 'danger',
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  body: ReactNode;
  confirmLabel: string;
  variant?: ButtonVariant;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  return (
    <Modal open={open} onClose={onCancel} label={title}>
      <Card className="flex flex-col gap-3 p-4">
        <h2 className="m-0 text-sm font-semibold">{title}</h2>
        <div className="m-0 text-sm text-muted">{body}</div>
        <div className="flex justify-end gap-2">
          <Button variant="secondary" autoFocus onClick={onCancel}>
            {t('cancel')}
          </Button>
          <Button variant={variant} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      </Card>
    </Modal>
  );
}
