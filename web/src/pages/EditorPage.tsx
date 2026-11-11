/**
 * Editor page: file tree, file editor, and a changes panel for one project.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge } from "../components/ui/Badge";
import { buttonClass } from "../components/ui/Button";
import { EditIcon, ProjectIcon } from "../components/ui/Icons";
import { InfoBar } from "../components/ui/InfoBar";
import { PageHeader } from "../components/ui/PageHeader";
import { SectionLabel } from "../components/ui/SectionLabel";
import { FileTree } from "../components/editor/FileTree";
import { EditorPane } from "../components/editor/EditorPane";
import { FileTabs } from "../components/editor/FileTabs";
import { ChangesPanel } from "../components/editor/ChangesPanel";
import { ProjectOpenActions } from "../components/project/ProjectOpenActions";
import {
  getProjectChanges,
  getProjectFiles,
  type Project,
  type ProjectChange,
  type ProjectFileListing,
} from "../lib/api";
import { buildFileTree } from "../lib/fileTree";
import { closeFileTab, openFileTab } from "../lib/fileTabs";
import { editorPath } from "../lib/route";
import { useT } from "../i18n";

export default function EditorPage({
  projects,
  projectId,
  path,
  sessionId,
}: {
  projects: Project[];
  projectId: string;
  path: string | null;
  sessionId: string | null;
}) {
  const t = useT();
  const [listing, setListing] = useState<ProjectFileListing | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [changes, setChanges] = useState<ProjectChange[]>([]);
  const [changesVersion, setChangesVersion] = useState(0);
  const [edited, setEdited] = useState<Set<string>>(() => new Set());
  const [editing, setEditing] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const [openFiles, setOpenFiles] = useState<string[]>(() => (path ? [path] : []));
  const [dirtyPaths, setDirtyPaths] = useState<Set<string>>(() => new Set());

  // The route can bring a file that is not in the list (browser back after a
  // close): the on-screen file always has its tab.
  useEffect(() => {
    if (path !== null) {
      setOpenFiles((previous) => (previous.includes(path) ? previous : [...previous, path]));
    }
  }, [path]);

  const project = projects.find((item) => item.id === projectId);
  const nodes = useMemo(
    () => (listing ? buildFileTree(listing.files) : []),
    [listing],
  );

  useEffect(() => {
    let cancelled = false;

    setLoading(true);
    setError(null);

    getProjectFiles(projectId, sessionId)
      .then((data) => {
        if (cancelled) return;
        setListing(data);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : t("unknownError"));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId, sessionId, t]);

  const refreshChanges = () => setChangesVersion((version) => version + 1);

  useEffect(() => {
    let cancelled = false;

    getProjectChanges(projectId)
      .then((data) => {
        if (cancelled) return;
        setChanges(data.files);
      })
      .catch(() => {
        if (cancelled) return;
        setChanges([]);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId, changesVersion]);

  const openFile = (file: string) => {
    setOpenFiles((previous) => openFileTab(previous, file));
    window.location.hash = editorPath(projectId, file, sessionId);
  };

  const closeFile = (file: string) => {
    const result = closeFileTab(openFiles, file);
    setOpenFiles(result.open);
    setDirtyPaths((previous) => {
      if (!previous.has(file)) return previous;
      const next = new Set(previous);
      next.delete(file);
      return next;
    });
    if (path !== null && file === path) {
      window.location.hash = result.fallback
        ? editorPath(projectId, result.fallback, sessionId)
        : editorPath(projectId, null, sessionId);
    }
  };

  const handleDirtyChange = useCallback(
    (dirty: boolean) => {
      if (path === null) return;
      // One entry per file: a pane's signal never clears a sibling's.
      setDirtyPaths((previous) => {
        if (previous.has(path) === dirty) return previous;
        const next = new Set(previous);
        if (dirty) next.add(path);
        else next.delete(path);
        return next;
      });
    },
    [path],
  );

  const handleSaved = (file: string) => {
    setEdited((prev) => {
      const next = new Set(prev);
      next.add(file);
      return next;
    });
    refreshChanges();
  };

  const handleRestored = (file: string) => {
    refreshChanges();
    if (path !== null && file === path) {
      setReloadToken((token) => token + 1);
    }
  };

  return (
    <div className="flex grow flex-col min-h-0 overflow-hidden p-4 gap-3">
      <PageHeader
        Icon={EditIcon}
        title={project?.name ?? projectId}
        subtitle={t("editorTitle")}
        actions={
          <>
            <a
              href={`#/project/${encodeURIComponent(projectId)}`}
              title={t("editorBackToProject")}
              aria-label={t("editorBackToProject")}
              className={buttonClass({
                variant: "ghost",
                size: "sm",
                iconOnly: true,
              })}
            >
              <ProjectIcon className="h-4 w-4" />
            </a>
            {project ? (
              <ProjectOpenActions
                projectId={projectId}
                editorUrl={project.editorUrl}
              />
            ) : null}
          </>
        }
      />

      {openFiles.length > 0 ? (
        <div className="flex min-h-12 shrink-0 items-stretch border-b border-border bg-panel px-3">
          <FileTabs
            tabs={openFiles.map((file) => ({
              path: file,
              name: file.slice(file.lastIndexOf("/") + 1),
              dirty: dirtyPaths.has(file),
            }))}
            activePath={path}
            onPick={openFile}
            onClose={closeFile}
          />
        </div>
      ) : null}

      {listing?.session ? (
        <InfoBar>
          <span className="flex flex-wrap items-center gap-2">
            {t("filesSessionVersion")}
            <a
              href={editorPath(projectId, path)}
              className={buttonClass({ variant: "ghost", size: "sm" })}
            >
              {t("filesSeeWorkingTree")}
            </a>
          </span>
        </InfoBar>
      ) : null}

      {loading ? (
        <p className="text-muted">{t("loading")}</p>
      ) : error ? (
        <p className="text-err">{error}</p>
      ) : (
        <div className="flex min-h-0 grow gap-3">
          <nav
            aria-label={t("editorFiles")}
            className="w-64 shrink-0 overflow-y-auto"
          >
            <SectionLabel className="px-2.5 pb-1 pt-0.5">
              <span className="flex items-center gap-2">
                {t("editorFiles")}
                <Badge tone="neutral">{listing?.files.length ?? 0}</Badge>
              </span>
            </SectionLabel>
            <FileTree nodes={nodes} selected={path} onSelect={openFile} />
            {listing?.truncated ? (
              <p className="px-2.5 pt-2 text-xs text-muted">
                {t("filesTruncated")}
              </p>
            ) : null}
          </nav>

          <div className="flex min-h-0 min-w-0 grow flex-col">
            {path ? (
              <EditorPane
                key={`${sessionId ?? ""}:${path}:${reloadToken}`}
                projectId={projectId}
                path={path}
                sessionId={sessionId}
                editing={editing && !listing?.session}
                onEditingChange={setEditing}
                onSaved={handleSaved}
                onDirtyChange={handleDirtyChange}
              />
            ) : (
              <p className="text-xs text-muted">{t("filesPickOne")}</p>
            )}
          </div>

          {listing && !listing.session ? (
            <ChangesPanel
              projectId={projectId}
              changes={changes}
              edited={edited}
              onOpen={openFile}
              onCommitted={refreshChanges}
              onRestored={handleRestored}
            />
          ) : null}
        </div>
      )}
    </div>
  );
}
