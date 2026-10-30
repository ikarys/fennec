import { useEffect, useState } from 'react';
import type { ServerDef } from '../../lib/api';
import { saveServers } from '../../lib/api';
import { CloseIcon, PlusIcon, SpinnerIcon, TrashIcon } from '../Icons';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { TextInput } from '../ui/Field';
import { Modal } from '../ui/Modal';

interface ServerEditorProps {
  open: boolean;
  sessionId: string;
  initial: ServerDef[];
  onSaved: () => void;
  onClose: () => void;
}

export function ServerEditor({ open, sessionId, initial, onSaved, onClose }: ServerEditorProps) {
  const [rows, setRows] = useState<ServerDef[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setRows(initial.map((server) => ({ name: server.name, command: server.command })));
    setError(null);
    setSaving(false);
  }, [open, initial]);

  if (!open) return null;

  const setRow = (index: number, field: keyof ServerDef, value: string) => {
    setRows((current) => current.map((row, i) => (i === index ? { ...row, [field]: value } : row)));
  };

  const addRow = () => setRows((current) => [...current, { name: '', command: '' }]);
  const removeRow = (index: number) => setRows((current) => current.filter((_, i) => i !== index));

  const trimmed = rows.map((row) => ({ name: row.name.trim(), command: row.command.trim() }));
  const names = trimmed.map((row) => row.name);
  const invalid =
    trimmed.some((row) => !row.name || !row.command) || new Set(names).size !== names.length;

  const handleSave = async () => {
    if (saving || invalid) return;
    setSaving(true);
    setError(null);
    try {
      await saveServers(sessionId, trimmed);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Erreur');
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} busy={saving} label="Configure les serveurs" width="w-[min(92vw,560px)]">
      <Card className="flex max-h-[85vh] w-full flex-col">
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-border p-4">
          <div className="flex min-w-0 flex-col gap-1">
            <h2 className="truncate text-base font-semibold text-text">Serveurs</h2>
            <p className="truncate text-sm text-muted">La liste du projet, dans .agents/servers.json.</p>
          </div>
          <Button variant="secondary" size="sm" iconOnly onClick={onClose} aria-label="Fermer">
            <CloseIcon className="h-4 w-4" />
          </Button>
        </header>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
          {rows.length === 0 && <div className="text-sm text-muted">Aucun serveur.</div>}
          {rows.map((row, index) => (
            <div key={index} className="flex items-center gap-2">
              <TextInput
                aria-label="Nom du serveur"
                value={row.name}
                onChange={(event) => setRow(index, 'name', event.target.value)}
                placeholder="web"
                className="w-28 shrink-0"
              />
              <TextInput
                aria-label="Commande du serveur"
                value={row.command}
                onChange={(event) => setRow(index, 'command', event.target.value)}
                placeholder="npm run dev"
                className="min-w-0 flex-1"
              />
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                aria-label="Supprimer ce serveur"
                onClick={() => removeRow(index)}
              >
                <TrashIcon className="h-3.5 w-3.5" />
              </Button>
            </div>
          ))}
          <Button variant="secondary" size="sm" onClick={addRow}>
            <PlusIcon className="h-3.5 w-3.5" />
            Ajouter un serveur
          </Button>
        </div>

        <footer className="flex shrink-0 flex-col gap-3 border-t border-border p-4">
          {error && <div className="text-sm text-err">{error}</div>}
          <div className="flex flex-wrap justify-end gap-2">
            <Button variant="secondary" size="sm" onClick={onClose}>
              Annuler
            </Button>
            <Button variant="primary" size="sm" disabled={saving || invalid} onClick={() => void handleSave()}>
              {saving ? (
                <>
                  <SpinnerIcon className="h-3.5 w-3.5" />
                  Enregistrement…
                </>
              ) : (
                'Enregistrer'
              )}
            </Button>
          </div>
        </footer>
      </Card>
    </Modal>
  );
}
