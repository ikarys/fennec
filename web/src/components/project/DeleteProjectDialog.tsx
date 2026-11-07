/** Asks the human to delete a project, showing exactly what is removed. */
import { useState } from "react";
import { useT } from "../../i18n";
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
  const t = useT();
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
          : t("deleteProjectError"),
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
      label={t("delete")}
    >
      <Card className="flex flex-col gap-3 p-4">
        <h2 className="m-0 text-sm font-semibold">
          {t("deleteProjectTitle", { name: project.name })}
        </h2>

        <ul className="m-0 list-disc space-y-1 pl-4 text-sm text-muted">
          <li>{t("deleteProjectSessions", { count: project.sessions.length })}</li>
          <li>{t("deleteProjectWorktrees")}</li>
          <li>{t("deleteProjectRules")}</li>
        </ul>

        <div className="flex flex-col gap-1">
          <label className="flex items-center gap-2 text-sm text-text">
            <input
              type="checkbox"
              checked={deleteFolder}
              onChange={(event) => setDeleteFolder(event.target.checked)}
              className="h-4 w-4"
            />
            {t("deleteProjectFolder")}
          </label>
          <code className="break-all font-mono text-xs text-muted">
            {project.workdir}
          </code>
          {deleteFolder && (
            <p className="m-0 text-xs text-err">
              {t("deleteProjectFolderWarning")}
            </p>
          )}
        </div>

        {folderError && <p className="m-0 text-sm text-err">{folderError}</p>}
        {thrown && <p className="m-0 text-sm text-err">{thrown}</p>}

        {folderError ? (
          <div className="flex justify-end">
            <Button variant="secondary" autoFocus onClick={onDeleted}>
              {t("close")}
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
              {t("cancel")}
            </Button>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() => void confirm()}
            >
              {busy
                ? t("deleting")
                : deleteFolder
                  ? t("deleteProjectFolderBtn")
                  : t("deleteProjectBtn")}
            </Button>
          </div>
        )}
      </Card>
    </Modal>
  );
}
