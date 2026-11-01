/** Asks the human to delete a project, showing exactly what is removed. */
import { useState } from "react";
import { deleteProject, type Project } from "../../lib/api";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";
import { Modal } from "../ui/Modal";

export function DeleteProjectDialog({
  project,
  onClose,
  onDeleted,
}: {
  project: Project;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [deleteFolder, setDeleteFolder] = useState(false);
  const [thrown, setThrown] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);

  const confirm = async () => {
    setBusy(true);
    setThrown(null);
    setFolderError(null);
    try {
      const result = await deleteProject(project.id, deleteFolder);
      if (result.folderError) {
        setFolderError(result.folderError);
      } else {
        onDeleted();
      }
    } catch (error) {
      setThrown(
        error instanceof Error
          ? error.message
          : "Impossible de supprimer le projet",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={folderError ? onDeleted : onClose}
      busy={busy}
      label="Supprimer"
    >
      <Card className="flex flex-col gap-3 p-4">
        <h2 className="m-0 text-sm font-semibold">
          Supprimer le projet {project.name}
        </h2>

        <ul className="m-0 list-disc space-y-1 pl-4 text-sm text-muted">
          <li>ses {project.sessions.length} sessions et leurs journaux</li>
          <li>les worktrees et branches fennec de ses sessions</li>
          <li>ses règles et ses idées</li>
        </ul>

        <div className="flex flex-col gap-1">
          <label className="flex items-center gap-2 text-sm text-text">
            <input
              type="checkbox"
              checked={deleteFolder}
              onChange={(event) => setDeleteFolder(event.target.checked)}
              className="h-4 w-4 accent-[var(--accent)]"
            />
            Supprimer aussi le dossier du code source
          </label>
          <code className="break-all font-mono text-xs text-muted">
            {project.workdir}
          </code>
          {deleteFolder && (
            <p className="m-0 text-xs text-err">
              Le dossier et tout son contenu seront effacés du disque, sans
              corbeille.
            </p>
          )}
        </div>

        {folderError && <p className="m-0 text-sm text-err">{folderError}</p>}
        {thrown && <p className="m-0 text-sm text-err">{thrown}</p>}

        {folderError ? (
          <div className="flex justify-end">
            <Button variant="secondary" autoFocus onClick={onDeleted}>
              Fermer
            </Button>
          </div>
        ) : (
          <div className="flex justify-end gap-2">
            <Button
              variant="secondary"
              autoFocus
              disabled={busy}
              onClick={onClose}
            >
              Annuler
            </Button>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() => void confirm()}
            >
              {busy
                ? "Suppression…"
                : deleteFolder
                  ? "Supprimer le projet et le dossier"
                  : "Supprimer le projet"}
            </Button>
          </div>
        )}
      </Card>
    </Modal>
  );
}
