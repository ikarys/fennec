/**
 * One file in the editor page: header, save/preview controls, and the CodeMirror view.
 */
import { useEffect, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { CheckIcon, EditIcon, EyeIcon, FileIcon, RefreshIcon } from '../ui/Icons';
import { markdownComponents } from '../session/markdownComponents';
import { CodeEditor } from './CodeEditor';
import {
  getProjectFile,
  saveProjectFile,
  type ProjectFileView,
} from '../../lib/api';
import { isMarkdown } from '../../lib/fileTree';
import { useT } from '../../i18n';

const REMARK_PLUGINS = [remarkGfm];

export function EditorPane({
  projectId,
  path,
  sessionId,
  onDirtyChange,
  onSaved,
}: {
  projectId: string;
  path: string;
  sessionId: string | null;
  onDirtyChange: (dirty: boolean) => void;
  onSaved: (path: string) => void;
}) {
  const t = useT();
  const [file, setFile] = useState<ProjectFileView | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [preview, setPreview] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const savedTimerRef = useRef<number | null>(null);

  const lastSlash = path.lastIndexOf('/');
  const dir = lastSlash >= 0 ? path.slice(0, lastSlash + 1) : '';
  const name = path.slice(lastSlash + 1);
  const canPreview = isMarkdown(path) && file !== null && !file.binary && !file.tooLarge;
  const dirty =
    file !== null &&
    draft !== null &&
    !file.binary &&
    !file.tooLarge &&
    draft !== (file.content ?? '');

  useEffect(() => {
    let cancelled = false;

    setLoading(true);
    setError(null);
    setSaveError(null);
    setFile(null);
    setDraft(null);
    setSaving(false);
    setSaved(false);
    setPreview(false);

    getProjectFile(projectId, path, sessionId)
      .then((data) => {
        if (cancelled) return;
        setFile(data);
        setDraft(data.content ?? '');
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
  }, [projectId, path, sessionId, reloadKey, t]);

  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    return () => {
      onDirtyChange(false);
      if (savedTimerRef.current !== null) {
        window.clearTimeout(savedTimerRef.current);
      }
    };
  }, [onDirtyChange]);

  const reload = () => {
    setSaveError(null);
    setReloadKey((key) => key + 1);
  };

  const save = async () => {
    if (!file || !dirty || saving || !file.editable) return;

    setSaving(true);
    setSaveError(null);

    try {
      const res = await saveProjectFile(projectId, {
        path,
        content: draft ?? '',
        base: file.blob ?? '',
      });

      setFile((prev) => (prev ? { ...prev, content: draft ?? '', blob: res.blob } : prev));
      onSaved(path);
      setSaved(true);

      if (savedTimerRef.current !== null) {
        window.clearTimeout(savedTimerRef.current);
      }
      savedTimerRef.current = window.setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="flex h-10 shrink-0 items-center gap-2">
        <FileIcon className="h-4 w-4 shrink-0 text-muted" />
        <span className="truncate font-mono text-xs">
          {dir ? <span className="text-muted">{dir}</span> : null}
          <span className="font-semibold text-text">{name}</span>
        </span>
        {file?.session ? <Badge tone="info">{t('filesSessionBadge')}</Badge> : null}
        {file && !file.editable && !file.session ? (
          <Badge tone="neutral">{t('editorReadOnly')}</Badge>
        ) : null}
        {dirty ? <Badge tone="attention">{t('editorUnsaved')}</Badge> : null}
        {saved ? <Badge tone="ok">{t('editorSaved')}</Badge> : null}

        <div className="ml-auto flex items-center gap-1">
          {canPreview ? (
            <Button
              type="button"
              variant="icon"
              size="sm"
              iconOnly
              title={preview ? t('editorSource') : t('editorPreview')}
              aria-label={preview ? t('editorSource') : t('editorPreview')}
              onClick={() => setPreview((value) => !value)}
            >
              {preview ? <EditIcon className="h-3.5 w-3.5" /> : <EyeIcon className="h-3.5 w-3.5" />}
            </Button>
          ) : null}
          <Button
            type="button"
            variant="icon"
            size="sm"
            iconOnly
            title={`${t('editorSave')} (Ctrl+S)`}
            aria-label={`${t('editorSave')} (Ctrl+S)`}
            disabled={!dirty || saving || !file?.editable}
            onClick={() => void save()}
          >
            <CheckIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      {saveError ? (
        <div className="flex shrink-0 items-center gap-2 text-xs text-err">
          <span>{saveError}</span>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            title={t('editorReload')}
            aria-label={t('editorReload')}
            onClick={reload}
          >
            <RefreshIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
      ) : null}

      {loading ? (
        <p className="text-muted">{t('loading')}</p>
      ) : error ? (
        <p className="text-err">{error}</p>
      ) : file ? (
        <>
          {file.binary ? (
            <p className="text-sm text-muted">{t('filesBinary')}</p>
          ) : file.tooLarge ? (
            <p className="text-sm text-muted">{t('filesTooLarge')}</p>
          ) : (
            <>
              {preview && canPreview ? (
                <div className="min-h-0 grow overflow-y-auto rounded-md border border-border p-5">
                  <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                    {draft ?? ''}
                  </ReactMarkdown>
                </div>
              ) : null}
              <div className={preview && canPreview ? 'hidden' : 'min-h-0 grow'}>
                <CodeEditor
                  value={file.content ?? ''}
                  path={path}
                  readOnly={!file.editable}
                  onChange={setDraft}
                  onSave={save}
                />
              </div>
            </>
          )}
        </>
      ) : null}
    </div>
  );
}
