/** Asks the human to delete the selected sessions, showing exactly what is removed and what is kept. */
import { useEffect, useRef, useState } from "react";
import { deleteSessions, type SessionSummary } from "../../lib/api";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";

export interface DeletionTarget {
  session: SessionSummary;
  projectName: string;
}

export function DeleteSessionsDialog({
  targets,
  onClose,
  onDeleted,
}: {
  targets: DeletionTarget[];
  onClose: () => void;
  onDeleted: (ids: string[]) => void;
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState<{ id: string; error: string }[]>([]);
  const [thrown, setThrown] = useState<string | null>(null);

  useEffect(() => dialogRef.current?.showModal(), []);

  const closeDialog = () => {
    dialogRef.current?.close();
    onClose();
  };

  const confirm = async () => {
    const ids = targets.map((target) => target.session.id);
    setBusy(true);
    setFailed([]);
    setThrown(null);
    try {
      const result = await deleteSessions(ids);
      if (result.deleted.length > 0) onDeleted(result.deleted);
      if (result.failed.length === 0) {
        closeDialog();
      } else {
        setFailed(result.failed);
      }
    } catch (error) {
      setThrown(
        error instanceof Error
          ? error.message
          : "Impossible de supprimer les sessions",
      );
    } finally {
      setBusy(false);
    }
  };

  const awaitingCount = targets.filter(
    (target) => target.session.awaitingIntegration,
  ).length;
  const titleById = new Map(
    targets.map((target): [string, string] => [
      target.session.id,
      target.session.title,
    ]),
  );

  return (
    <dialog
      ref={dialogRef}
      aria-label="Supprimer"
      onCancel={(event) => {
        if (busy) event.preventDefault();
        else closeDialog();
      }}
      onClick={(event) => {
        if (event.target === dialogRef.current && !busy) closeDialog();
      }}
      className="m-auto w-[min(92vw,560px)] text-text backdrop:bg-black/60"
    >
      <Card className="flex flex-col gap-3 p-4">
        <h2 className="m-0 text-sm font-semibold">
          {targets.length === 1
            ? "Supprimer la session"
            : `Supprimer ${targets.length} sessions`}
        </h2>

        <ul className="max-h-56 overflow-y-auto rounded-md border border-border">
          {targets.map((target) => (
            <li
              key={target.session.id}
              className="flex items-center gap-2 border-b border-border px-3 py-1.5 last:border-b-0 text-sm"
            >
              <span className="min-w-0 grow truncate">
                {target.session.title}
              </span>
              <span className="shrink-0 text-xs text-muted">
                {target.projectName}
              </span>
              {target.session.awaitingIntegration && (
                <Badge tone="accent">à intégrer</Badge>
              )}
            </li>
          ))}
        </ul>

        {awaitingCount > 0 && (
          <div className="rounded-md border border-err/40 bg-del-bg px-3 py-2 text-sm text-err">
            {awaitingCount === 1
              ? "1 session a du travail non intégré : ses commits seront perdus. Intègre-la d'abord pour les garder."
              : `${awaitingCount} sessions ont du travail non intégré : leurs commits seront perdus. Intègre-les d'abord pour les garder.`}
          </div>
        )}

        <p className="m-0 text-xs text-muted">
          Supprimés : le fil, les étapes de workflow, l'espace de l'agent et sa
          branche git. Gardés : les chiffres d'usage (Insight). C'est
          définitif.
        </p>

        {failed.length > 0 && (
          <div className="flex flex-col gap-1">
            {failed.map((item) => (
              <p key={item.id} className="m-0 text-sm text-err">
                « {titleById.get(item.id) ?? item.id} » : {item.error}
              </p>
            ))}
          </div>
        )}
        {thrown && <p className="m-0 text-sm text-err">{thrown}</p>}

        <div className="flex justify-end gap-2">
          <Button
            variant="secondary"
            autoFocus
            disabled={busy}
            onClick={closeDialog}
          >
            Annuler
          </Button>
          <Button
            variant="danger"
            disabled={busy}
            onClick={() => void confirm()}
          >
            {busy ? "Suppression…" : "Supprimer"}
          </Button>
        </div>
      </Card>
    </dialog>
  );
}
