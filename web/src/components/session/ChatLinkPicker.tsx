// The chat's project link: a reference for grouping chats, the chat stays without folder or git.
import { useEffect, useState } from 'react';
import type { Project } from '../../lib/api';
import { linkSession } from '../../lib/api';
import { useT } from '../../i18n';
import { ChoiceMenu } from '../ui/ChoiceMenu';
import { FolderIcon } from '../ui/Icons';

export function ChatLinkPicker({ sessionId, linkedProjectId, projects, onLinked }: { sessionId: string; linkedProjectId: string | null; projects: Project[]; onLinked: () => void }) {
  const t = useT();
  const [value, setValue] = useState<string | null>(linkedProjectId);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setValue(linkedProjectId);
    setError(null);
  }, [linkedProjectId]);

  const linkedProject = projects.find((project) => project.id === value);

  return (
    <div className="flex items-center gap-2">
      <ChoiceMenu
        direction="down"
        align="right"
        label={t('chatLinkLabel')}
        title={t('chatLinkLabel')}
        value={value ?? ''}
        onChange={(selected) => {
          const target = selected === '' ? null : selected;
          linkSession(sessionId, target)
            .then((result) => {
              setValue(result.linkedProjectId);
              setError(null);
              onLinked();
            })
            .catch((failure: unknown) => {
              setError(failure instanceof Error ? failure.message : String(failure));
            });
        }}
        groups={[
          {
            choices: [{ value: '', label: t('chatLinkNone') }],
          },
          {
            label: t('paletteProjects'),
            choices: projects
              .filter((project) => !project.chat && !project.sessionWorktree)
              .map((project) => ({
                value: project.id,
                label: project.name,
                description: project.workdir,
              })),
          },
        ]}
        trigger={
          <>
            <FolderIcon className="h-3.5 w-3.5" />
            <span className="max-w-[10rem] truncate">{linkedProject?.name ?? t('chatLinkPrompt')}</span>
          </>
        }
      />
      {error ? <span className="text-xs text-err">{error}</span> : null}
    </div>
  );
}
