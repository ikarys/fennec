import { useCallback, useEffect, useState } from 'react';
import { getProjects } from '../lib/api';
import type { Project } from '../lib/api';
import { t } from '../i18n/messages';
import type { Lang } from '../i18n/messages';

export function useProjects(language: Lang) {
  // `projectsLoading` is only the first load: a refresh keeps the list on
  // screen instead of flashing a loading state.
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);
  const [projectsError, setProjectsError] = useState<string | null>(null);

  const refreshProjects = useCallback(async () => {
    try {
      const data = await getProjects();
      // An unchanged list keeps its identity, so nothing keyed on it reloads.
      setProjects((current) => (JSON.stringify(current) === JSON.stringify(data) ? current : data));
      setProjectsError(null);
    } catch (error) {
      setProjectsError(error instanceof Error ? error.message : t(language, 'projectsLoadError'));
    } finally {
      setProjectsLoading(false);
    }
  }, [language]);

  useEffect(() => {
    refreshProjects();
  }, [refreshProjects]);

  return { projects, projectsLoading, projectsError, refreshProjects };
}
