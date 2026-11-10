// Session selection shared by the projects and chats pages: which sessions are
// selected, how to toggle/clear them, how to open the delete dialog, and the
// dialog element itself for the page to render.
import { useState } from 'react';
import type { Project } from '../lib/api';
import { DeleteSessionsDialog, type DeletionTarget } from '../components/session/DeleteSessionsDialog';

export function useSessionSelection(projects: Project[], onSessionsDeleted: (ids: string[]) => void) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<DeletionTarget[] | null>(null);

  const toggle = (id: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const clear = () => {
    setSelected(new Set());
  };

  const remove = (ids: string[]) => {
    const targets = projects.flatMap((project) =>
      project.sessions
        .filter((session) => ids.includes(session.id))
        .map((session) => ({ session, projectName: project.name }))
    );
    if (targets.length > 0) setPendingDelete(targets);
  };

  const dialog =
    pendingDelete ? (
      <DeleteSessionsDialog
        targets={pendingDelete}
        onClose={() => setPendingDelete(null)}
        onDeleted={(ids) => {
          onSessionsDeleted(ids);
          setSelected((previous) => {
            const next = new Set(previous);
            for (const id of ids) next.delete(id);
            return next;
          });
        }}
      />
    ) : null;

  return { selected, toggle, clear, remove, dialog };
}
