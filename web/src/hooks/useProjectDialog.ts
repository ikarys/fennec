// The "open a project" dialog: its open state, and the creation call with its pending and error state.
import { useCallback, useState } from 'react';
import { createProject } from '../lib/api';
import { t } from '../i18n/messages';
import type { Lang } from '../i18n/messages';

export function useProjectDialog(language: Lang, refreshProjects: () => Promise<void>) {
  const [open, setOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openProjectDialog = useCallback(() => setOpen(true), []);

  const onSubmit = useCallback(
    async (workdir: string) => {
      setSubmitting(true);
      setError(null);
      try {
        await createProject(workdir);
        setOpen(false);
        await refreshProjects();
      } catch (caught) {
        setError(caught instanceof Error ? caught.message : t(language, 'projectOpenError'));
      } finally {
        setSubmitting(false);
      }
    },
    [refreshProjects, language]
  );

  return { openProjectDialog, dialogProps: { open, submitting, error, onClose: () => setOpen(false), onSubmit } };
}
