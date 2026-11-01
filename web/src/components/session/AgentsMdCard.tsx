/**
 * The one-click write of the agent's AGENTS.md draft into the project root.
 *
 * Agents read AGENTS.md but never write it: the human confirms and fennec
 * writes the file in the human's tree, without committing it.
 */
import { useState } from 'react';

import { writeAgentsMd } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { ConfirmDialog } from '../ui/ConfirmDialog';

export function AgentsMdCard({
  draft,
  sessionId,
  onWritten,
}: {
  draft: { exists: boolean; written: boolean };
  sessionId: string | null;
  onWritten: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!sessionId) return null;

  if (draft.written) {
    return (
      <Card className="p-4">
        <p className="m-0 text-sm text-muted">
          AGENTS.md écrit depuis ce brouillon. Relis-le et commite-le.
        </p>
      </Card>
    );
  }

  const confirm = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await writeAgentsMd(sessionId);
      setOpen(false);
      onWritten();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Erreur inconnue');
      setOpen(false);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-1">
        <p className="m-0 text-sm font-medium text-text">Écrire le brouillon dans AGENTS.md ?</p>
        <p className="m-0 text-xs text-muted">
          Les agents lisent AGENTS.md mais ne peuvent pas l'écrire : c'est toi qui fais entrer ces
          règles dans le projet. Le fichier est écrit à la racine, sans commit.
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Button variant="primary" size="md" onClick={() => setOpen(true)} disabled={busy}>
          Écrire AGENTS.md
        </Button>
      </div>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      <ConfirmDialog
        open={open}
        title="Écrire AGENTS.md"
        body={
          draft.exists
            ? "Un AGENTS.md existe déjà : il sera remplacé par le brouillon de l'agent. Relis le brouillon ci-dessus avant."
            : "AGENTS.md sera créé à la racine du projet avec le brouillon ci-dessus."
        }
        confirmLabel={draft.exists ? 'Remplacer' : 'Écrire'}
        variant={draft.exists ? 'danger' : undefined}
        onConfirm={confirm}
        onCancel={() => setOpen(false)}
      />
    </Card>
  );
}
