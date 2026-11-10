/**
 * One file in the editor page: header, edit toggle, and the CodeMirror view.
 */
import { useEffect, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { EditIcon, FileIcon, RefreshIcon } from '../ui/Icons';
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
  editing,
  onEditingChange,
  onSaved,
}: {
  projectId: string;
  path: string;
  sessionId: string | null;
  editing: boolean;
  onEditingChange: (editing: boolean) => void;
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
  const [paused, setPaused] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const timerRef = useRef<number | null>(null);
  const savedTimerRef = useRef<number | null>(null);
  const savingRef = useRef(false);
  const latestRef = useRef<{ file: ProjectFileView | null; draft: string | null; paused: boolean }>({
    file: null,
    draft: null,
    paused: false,
  });
  const unmountRef = useRef<() => void>(() => {});

  const lastSlash = path.lastIndexOf('/');
  const dir = lastSlash >= 0 ? path.slice(0, lastSlash + 1) : '';
  const name = path.slice(lastSlash + 1);
  const canMarkdown = isMarkdown(path) && file !== null && !file.binary && !file.tooLarge;
  const dirty =
    file !== null &&
    draft !== null &&
    !file.binary &&
    !file.tooLarge &&
    draft !== (file.content ?? '');

  useEffect(() => {
    latestRef.current = { file, draft, paused };
  }, [file, draft, paused]);

  useEffect(() => {
    savingRef.current = saving;
  }, [saving]);

  useEffect(() => {
    let cancelled = false;

    setLoading(true);
    setError(null);
    setSaveError(null);
    setFile(null);
    setDraft(null);
    setSaving(false);
    setSaved(false);
    setPaused(false);

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
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      if (savedTimerRef.current !== null) {
        window.clearTimeout(savedTimerRef.current);
        savedTimerRef.current = null;
      }
    };
  }, [projectId, path, sessionId, reloadKey, t]);

  const clearTimer = () => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  };

  const save = async () => {
    clearTimer();

    if (!file || draft === null || saving || !file.editable || file.binary || file.tooLarge) return;
    if (draft === (file.content ?? '')) return;

    const content = draft;
    const base = file.blob ?? '';

    setSaving(true);
    setSaveError(null);

    try {
      const res = await saveProjectFile(projectId, { path, content, base });

      setFile((prev) => (prev ? { ...prev, content, blob: res.blob } : prev));
      onSaved(path);
      setSaved(true);

      if (savedTimerRef.current !== null) {
        window.clearTimeout(savedTimerRef.current);
      }
      savedTimerRef.current = window.setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : t('unknownError'));
      setPaused(true);
    } finally {
      setSaving(false);
    }
  };

  unmountRef.current = () => {
    const { file: latestFile, draft: latestDraft, paused: latestPaused } = latestRef.current;

    if (!latestFile || latestDraft === null || latestPaused || savingRef.current) return;
    if (latestFile.binary || latestFile.tooLarge || !latestFile.editable) return;
    if (latestDraft === (latestFile.content ?? '')) return;

    void saveProjectFile(projectId, {
      path,
      content: latestDraft,
      base: latestFile.blob ?? '',
    })
      .then(() => onSaved(path))
      .catch(() => {});
  };

  useEffect(() => () => unmountRef.current(), []);

  useEffect(() => {
    if (!editing) {
      void save();
    }
  }, [editing]);

  useEffect(() => {
    if (!editing || !file || !file.editable || !dirty || saving || paused) return;

    timerRef.current = window.setTimeout(() => {
      void save();
    }, 800);

    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [draft, file, editing, saving, paused]);

  useEffect(() => {
    if (!dirty) return;

    const handler = (event: BeforeUnloadEvent) => {
      event.preventDefault();
    };

    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [dirty]);

  const reload = () => {
    setSaveError(null);
    setPaused(false);
    setReloadKey((key) => key + 1);
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
        {saving ? <Badge tone="neutral">{t('editorSaving')}</Badge> : null}
        {saved ? <Badge tone="ok">{t('editorSaved')}</Badge> : null}

        <div className="ml-auto flex items-center gap-1">
          {file?.editable ? (
            <Button
              type="button"
              variant={editing ? 'soft' : 'icon'}
              size="sm"
              iconOnly
              aria-pressed={editing}
              title={t(editing ? 'editorStopEditing' : 'editorEdit')}
              aria-label={t(editing ? 'editorStopEditing' : 'editorEdit')}
              onClick={() => onEditingChange(!editing)}
            >
              <EditIcon className="h-3.5 w-3.5" />
            </Button>
          ) : null}
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
              {!editing && canMarkdown ? (
                <div className="min-h-0 grow overflow-y-auto rounded-md border border-border p-5">
                  <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                    {draft ?? file.content ?? ''}
                  </ReactMarkdown>
                </div>
              ) : (
                <div className="min-h-0 grow">
                  <CodeEditor
                    value={draft ?? file.content ?? ''}
                    path={path}
                    readOnly={!editing || !file.editable}
                    onChange={setDraft}
                    onSave={() => void save()}
                  />
                </div>
              )}
            </>
          )}
        </>
      ) : null}
    </div>
  );
}
