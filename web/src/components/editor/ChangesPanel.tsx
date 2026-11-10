/**
 * The editor page's changes panel: ticked files, one message, one commit.
 */
import { useEffect, useState } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { CheckIcon, ReplayIcon } from '../ui/Icons';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { SectionLabel } from '../ui/SectionLabel';
import { TextInput } from '../ui/Field';
import {
  commitProjectFiles,
  restoreProjectFile,
  type ProjectChange,
} from '../../lib/api';
import { defaultCommitMessage } from '../../lib/fileTree';
import { useT } from '../../i18n';

export function ChangesPanel({
  projectId,
  changes,
  edited,
  onOpen,
  onCommitted,
  onRestored,
}: {
  projectId: string;
  changes: ProjectChange[];
  edited: Set<string>;
  onOpen: (path: string) => void;
  onCommitted: () => void;
  onRestored: (path: string) => void;
}) {
  const t = useT();
  const [ticked, setTicked] = useState<Set<string>>(() => new Set());
  const [message, setMessage] = useState('');
  const [touched, setTouched] = useState(false);
  const [committing, setCommitting] = useState(false);
  const [commitError, setCommitError] = useState<string | null>(null);
  const [committed, setCommitted] = useState<string | null>(null);
  const [restorePath, setRestorePath] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);

  useEffect(() => {
    setTicked((prev) => {
      const next = new Set<string>();
      for (const change of changes) {
        if (prev.has(change.path) || edited.has(change.path)) {
          next.add(change.path);
        }
      }
      return next;
    });
  }, [changes, edited]);

  useEffect(() => {
    if (!touched) {
      setMessage(defaultCommitMessage([...ticked]));
    }
  }, [ticked, touched]);

  const toggle = (path: string) => {
    setTicked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const commit = async () => {
    if (ticked.size === 0 || message.trim().length === 0 || committing) return;

    setCommitting(true);
    setCommitError(null);
    setCommitted(null);

    try {
      const res = await commitProjectFiles(projectId, {
        paths: [...ticked],
        message: message.trim(),
      });

      setCommitted(res.commit);
      setTicked(new Set());
      setTouched(false);
      onCommitted();
    } catch (err) {
      setCommitError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setCommitting(false);
    }
  };

  const restore = async () => {
    if (!restorePath || restoring) return;

    setRestoring(true);
    setRestoreError(null);

    try {
      await restoreProjectFile(projectId, restorePath);
      onRestored(restorePath);
    } catch (err) {
      setRestoreError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setRestoring(false);
      setRestorePath(null);
    }
  };

  return (
    <aside className="flex min-h-0 w-72 shrink-0 flex-col rounded-md border border-border bg-panel">
      <div className="shrink-0 border-b border-border p-3">
        <SectionLabel>
          <span className="flex items-center gap-2">
            {t('editorChanges')}
            <Badge tone="neutral">{changes.length}</Badge>
          </span>
        </SectionLabel>
      </div>

      {changes.length === 0 ? (
        <p className="p-3 text-xs text-muted">{t('editorNoChanges')}</p>
      ) : (
        <>
          <div className="min-h-0 grow overflow-y-auto">
            {changes.map((change) => (
              <div
                key={change.path}
                className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-raised"
              >
                <input
                  type="checkbox"
                  className="accent-[var(--accent)]"
                  aria-label={change.path}
                  checked={ticked.has(change.path)}
                  onChange={() => toggle(change.path)}
                />
                <Badge
                  tone={
                    change.status === 'modified'
                      ? 'attention'
                      : change.status === 'added'
                        ? 'ok'
                        : 'err'
                  }
                >
                  {change.status === 'modified' ? 'M' : change.status === 'added' ? 'A' : 'D'}
                </Badge>
                {change.status !== 'deleted' ? (
                  <button
                    type="button"
                    className="min-w-0 truncate font-mono text-xs"
                    title={change.path}
                    onClick={() => onOpen(change.path)}
                  >
                    {change.path}
                  </button>
                ) : (
                  <span
                    className="min-w-0 truncate font-mono text-xs text-muted"
                    title={change.path}
                  >
                    {change.path}
                  </span>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  iconOnly
                  className="ml-auto"
                  title={t('editorRestore')}
                  aria-label={t('editorRestore')}
                  onClick={() => setRestorePath(change.path)}
                >
                  <ReplayIcon className="h-3.5 w-3.5" />
                </Button>
              </div>
            ))}
          </div>

          {restoreError ? <p className="px-3 py-2 text-xs text-err">{restoreError}</p> : null}

          <div className="flex shrink-0 flex-col gap-2 border-t border-border p-3">
            <TextInput
              value={message}
              onChange={(event) => {
                setTouched(true);
                setMessage(event.target.value);
              }}
              placeholder={t('filesCommitMessage')}
            />
            <Button
              type="button"
              variant="primary"
              className="w-full"
              disabled={ticked.size === 0 || message.trim().length === 0 || committing}
              onClick={() => void commit()}
            >
              <CheckIcon className="h-3.5 w-3.5" />
              {t('filesCommit')}
            </Button>
            {committed ? (
              <Badge tone="ok" className="w-fit">
                {t('filesCommitted', { commit: committed.slice(0, 7) })}
              </Badge>
            ) : null}
            {commitError ? <p className="text-xs text-err">{commitError}</p> : null}
          </div>
        </>
      )}

      <ConfirmDialog
        open={restorePath !== null}
        title={t('editorRestoreTitle')}
        body={t('editorRestoreBody')}
        confirmLabel={t('editorRestoreConfirm')}
        variant="danger"
        onConfirm={() => void restore()}
        onCancel={() => setRestorePath(null)}
      />
    </aside>
  );
}
