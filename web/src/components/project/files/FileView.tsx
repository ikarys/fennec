/**
 * One file in the project's Files tab: rendered markdown or monospace, with
 * a commit editor for tracked files.
 */
import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { Section } from '../../ui/Section';
import { TextArea, TextInput } from '../../ui/Field';
import { CheckIcon, CloseIcon, EditIcon, FileIcon } from '../../ui/Icons';
import { markdownComponents } from '../../session/markdownComponents';
import {
  commitProjectFile,
  getProjectFile,
  type ProjectFileView,
} from '../../../lib/api';
import { defaultCommitMessage, isMarkdown } from '../../../lib/fileTree';
import { useT } from '../../../i18n';

const REMARK_PLUGINS = [remarkGfm];

export function FileView({
  projectId,
  path,
  sessionId,
}: {
  projectId: string;
  path: string;
  sessionId: string | null;
}) {
  const [file, setFile] = useState<ProjectFileView | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<string | null>(null);
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);
  const [commitError, setCommitError] = useState<string | null>(null);
  const [lastCommit, setLastCommit] = useState<string | null>(null);
  const t = useT();

  useEffect(() => {
    let cancelled = false;

    setLoading(true);
    setError(null);
    setEditing(false);
    setDraft(null);
    setMessage('');
    setSaving(false);
    setCommitError(null);
    setLastCommit(null);

    getProjectFile(projectId, path, sessionId)
      .then((data) => {
        if (cancelled) return;
        setFile(data);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : t('unknownError'));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId, path, sessionId]);

  const startEdit = () => {
    if (!file) return;
    setEditing(true);
    setDraft(file.content ?? '');
    setMessage(defaultCommitMessage(path));
    setCommitError(null);
  };

  const cancelEdit = () => {
    setEditing(false);
    setDraft(null);
    setCommitError(null);
  };

  const handleCommit = async () => {
    if (!file || draft === null || saving) return;

    setSaving(true);
    setCommitError(null);

    try {
      const res = await commitProjectFile(projectId, {
        path,
        content: draft,
        base: file.blob ?? '',
        message: message.trim(),
      });

      const fresh = await getProjectFile(projectId, path, sessionId);
      setFile(fresh);
      setEditing(false);
      setDraft(null);
      setLastCommit(res.commit);
    } catch (err) {
      setCommitError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setSaving(false);
    }
  };

  const fileName = path.slice(path.lastIndexOf('/') + 1);

  return (
    <Section
      Icon={FileIcon}
      title={fileName}
      flush
      actions={
        <div className="flex items-center gap-2">
          {file?.session ? <Badge tone="info">{t('filesSessionBadge')}</Badge> : null}
          {lastCommit ? (
            <Badge tone="ok">
              {t('filesCommitted', { commit: lastCommit.slice(0, 7) })}
            </Badge>
          ) : null}
          {file?.editable && !editing ? (
            <Button
              type="button"
              variant="icon"
              size="sm"
              iconOnly
              title={t('filesEdit')}
              aria-label={t('filesEdit')}
              onClick={startEdit}
            >
              <EditIcon className="h-3.5 w-3.5" />
            </Button>
          ) : null}
        </div>
      }
    >
      <div className="px-5 py-4">
        <p className="text-xs text-muted font-mono">{path}</p>

        {loading ? (
          <p className="mt-2 text-sm text-muted">{t('loading')}</p>
        ) : error ? (
          <p className="mt-2 text-sm text-err">{error}</p>
        ) : file ? (
          <>
            {file.binary ? (
              <p className="mt-2 text-sm text-muted">{t('filesBinary')}</p>
            ) : file.tooLarge ? (
              <p className="mt-2 text-sm text-muted">{t('filesTooLarge')}</p>
            ) : editing ? (
              <div className="mt-2 flex flex-col gap-3">
                <TextArea
                  value={draft ?? ''}
                  onChange={(event) => setDraft(event.target.value)}
                  className="font-mono text-xs"
                  rows={24}
                  spellCheck={false}
                />
                <div className="flex items-center gap-2">
                  <TextInput
                    value={message}
                    onChange={(event) => setMessage(event.target.value)}
                    placeholder={t('filesCommitMessage')}
                    className="min-w-0 grow"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    iconOnly
                    title={t('cancel')}
                    aria-label={t('cancel')}
                    onClick={cancelEdit}
                  >
                    <CloseIcon className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    type="button"
                    variant="primary"
                    size="sm"
                    disabled={
                      saving ||
                      message.trim().length === 0 ||
                      draft === file.content
                    }
                    onClick={() => void handleCommit()}
                  >
                    <CheckIcon className="h-3.5 w-3.5" />
                    {t('filesCommit')}
                  </Button>
                </div>
                {commitError ? (
                  <p className="text-xs text-err">{commitError}</p>
                ) : null}
              </div>
            ) : isMarkdown(path) ? (
              <div className="mt-2">
                <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                  {file.content ?? ''}
                </ReactMarkdown>
              </div>
            ) : (
              <pre className="overflow-x-auto rounded-md bg-code p-3 font-mono text-xs leading-relaxed text-text">
                {file.content ?? ''}
              </pre>
            )}
          </>
        ) : null}
      </div>
    </Section>
  );
}
