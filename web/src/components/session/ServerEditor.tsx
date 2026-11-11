import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { ServerDef } from '../../lib/api';
import { saveServers } from '../../lib/api';
import { CloseIcon, SpinnerIcon, TrashIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { TextInput } from '../ui/Field';
import { Modal } from '../ui/Modal';

interface ServerEditorProps {
  open: boolean;
  sessionId: string;
  servers: ServerDef[];
  editing: string | null;
  onSaved: () => void;
  onClose: () => void;
}

export function ServerEditor({
  open,
  sessionId,
  servers,
  editing,
  onSaved,
  onClose
}: ServerEditorProps) {
  const t = useT();
  const [name, setName] = useState('');
  const [command, setCommand] = useState('');
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wasOpen = useRef(false);

  useEffect(() => {
    if (open && !wasOpen.current) {
      const found =
        editing !== null ? servers.find((server) => server.name === editing) : undefined;
      setName(found?.name ?? '');
      setCommand(found?.command ?? '');
      setError(null);
      setSaving(false);
      setDeleting(false);
    }
    wasOpen.current = open;
  }, [open, editing, servers]);

  if (!open) return null;

  const trimmedName = name.trim();
  const trimmedCommand = command.trim();
  const duplicate =
    trimmedName !== '' &&
    servers.some((server) => server.name === trimmedName && server.name !== editing);
  const invalid = trimmedName === '' || trimmedCommand === '' || duplicate;
  const busy = saving || deleting;
  const title =
    editing !== null ? t('serverEdit', { name: editing }) : t('serverAdd');
  const message = error ?? (duplicate ? t('serverNameTaken') : null);

  const handleSave = async () => {
    if (busy || invalid) return;
    setSaving(true);
    setError(null);

    try {
      const list =
        editing === null
          ? [
              ...servers.map((server) => ({ name: server.name, command: server.command })),
              { name: trimmedName, command: trimmedCommand }
            ]
          : servers.map((server) =>
              server.name === editing
                ? { name: trimmedName, command: trimmedCommand }
                : { name: server.name, command: server.command }
            );
      await saveServers(sessionId, list);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('error'));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (busy || editing === null) return;
    setDeleting(true);
    setError(null);

    try {
      await saveServers(
        sessionId,
        servers
          .filter((server) => server.name !== editing)
          .map((server) => ({ name: server.name, command: server.command }))
      );
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('error'));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} busy={busy} label={title} width="w-[min(92vw,560px)]">
      <Card className="flex max-h-[85vh] w-full flex-col">
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-border p-4">
          <h2 className="truncate text-base font-semibold text-text">{title}</h2>
          <Button variant="secondary" size="sm" iconOnly onClick={onClose} aria-label={t('close')}>
            <CloseIcon className="h-4 w-4" />
          </Button>
        </header>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
          <TextInput
            aria-label={t('serverEditorNameLabel')}
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t('serverEditorNamePlaceholder')}
            className="w-full"
          />
          <TextInput
            aria-label={t('serverEditorCommandLabel')}
            value={command}
            onChange={(event) => setCommand(event.target.value)}
            placeholder={t('serverEditorCommandPlaceholder')}
            className="w-full"
          />
        </div>

        <footer className="flex shrink-0 flex-col gap-3 border-t border-border p-4">
          {message !== null && <div className="text-sm text-err">{message}</div>}
          <div className="flex flex-wrap items-center justify-between gap-2">
            {editing !== null ? (
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                disabled={busy}
                aria-label={t('serverEditorRemoveServer')}
                title={t('serverEditorRemoveServer')}
                onClick={() => void handleDelete()}
              >
                {deleting ? (
                  <SpinnerIcon className="h-3.5 w-3.5" />
                ) : (
                  <TrashIcon className="h-3.5 w-3.5" />
                )}
              </Button>
            ) : (
              <span />
            )}
            <div className="flex flex-wrap justify-end gap-2">
              <Button variant="secondary" size="sm" disabled={busy} onClick={onClose}>
                {t('cancel')}
              </Button>
              <Button
                variant="primary"
                size="sm"
                disabled={busy || invalid}
                onClick={() => void handleSave()}
              >
                {saving ? (
                  <>
                    <SpinnerIcon className="h-3.5 w-3.5" />
                    {t('sectionSaving')}
                  </>
                ) : (
                  t('agentSave')
                )}
              </Button>
            </div>
          </div>
        </footer>
      </Card>
    </Modal>
  );
}
