// The dialog that adds a server, or edits or removes one, in the project's .agents/servers.json.
import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { ServerDef } from '../../lib/api';
import { saveServers } from '../../lib/api';
import { CloseIcon, ServerIcon, SpinnerIcon, TrashIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { FieldRow, TextInput } from '../ui/Field';
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
    <Modal open={open} onClose={onClose} busy={busy} label={title} width="w-[min(92vw,480px)]">
      <Card className="flex max-h-[85vh] w-full flex-col">
        <header className="flex shrink-0 items-center gap-3 border-b border-border bg-gradient-to-r from-accent-soft to-transparent px-4 py-3">
          <div className="grid size-8 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
            <ServerIcon className="h-4 w-4" />
          </div>
          <div className="flex min-w-0 flex-col">
            <h2 className="truncate text-sm font-semibold text-text">{title}</h2>
            <p className="m-0 truncate text-xs text-muted">{t('serverEditorFileHint')}</p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            className="ml-auto"
            title={t('close')}
            aria-label={t('close')}
            onClick={onClose}
          >
            <CloseIcon className="h-3.5 w-3.5" />
          </Button>
        </header>

        <form
          className="flex min-h-0 flex-1 flex-col"
          onSubmit={(event) => {
            event.preventDefault();
            void handleSave();
          }}
        >
          <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
            <FieldRow
              label={t('serverEditorNameLabel')}
              htmlFor="server-editor-name"
              hint={
                duplicate ? <span className="text-err">{t('serverNameTaken')}</span> : undefined
              }
            >
              <TextInput
                id="server-editor-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder={t('serverEditorNamePlaceholder')}
                className="w-full"
                autoFocus={editing === null}
                aria-invalid={duplicate}
              />
            </FieldRow>
            <FieldRow
              label={t('serverEditorCommandLabel')}
              htmlFor="server-editor-command"
              hint={
                <>
                  {t('serversStartedNote')} <code className="font-mono text-text">$PORT</code>.
                </>
              }
            >
              <TextInput
                id="server-editor-command"
                value={command}
                onChange={(event) => setCommand(event.target.value)}
                placeholder={t('serverEditorCommandPlaceholder')}
                className="w-full font-mono"
                autoFocus={editing !== null}
              />
            </FieldRow>
            {error !== null && <p className="m-0 text-xs text-err">{error}</p>}
          </div>

          <footer className="flex shrink-0 items-center gap-2 border-t border-border bg-raised/40 px-4 py-3">
            {editing !== null && (
              <Button
                type="button"
                variant="danger"
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
            )}
            <div className="ml-auto flex gap-2">
              <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={onClose}>
                {t('cancel')}
              </Button>
              <Button type="submit" variant="primary" size="sm" disabled={busy || invalid}>
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
          </footer>
        </form>
      </Card>
    </Modal>
  );
}
