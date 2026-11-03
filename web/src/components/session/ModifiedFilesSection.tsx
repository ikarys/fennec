import { useEffect, useMemo, useState } from 'react';
import type { ThreadView } from '../../lib/fold';
import { previewKindOfPath, relativeToWorkdir, type PreviewKind } from '../../lib/preview';
import { usePreviewOpener, usePreviewWorkdir } from '../preview/PreviewContext';
import { panelBlock } from '../../lib/panelBlocks';
import { EyeIcon, FileIcon } from '../Icons';
import { Button } from '../ui/Button';
import { Panel } from '../ui/Panel';

interface ModifiedFilesSectionProps {
  modifiedFiles: ThreadView['modifiedFiles'];
  sessionId: string | null;
}

export function ModifiedFilesSection({ modifiedFiles, sessionId }: ModifiedFilesSectionProps) {
  const opener = usePreviewOpener();
  const workdir = usePreviewWorkdir();
  const [filesExpanded, setFilesExpanded] = useState(false);

  useEffect(() => {
    // A tab change collapses the file list.
    setFilesExpanded(false);
  }, [sessionId]);

  const mergedFiles = useMemo(() => {
    const merged = new Map<string, { key: string; added: number; removed: number; kind?: PreviewKind }>();
    for (const file of modifiedFiles) {
      // The same file arrives absolute and relative: one entry per project path.
      const key = (workdir ? relativeToWorkdir(file.path, workdir) : undefined) ?? file.path;
      const entry: { key: string; added: number; removed: number; kind?: PreviewKind } = merged.get(key) ?? { key, added: 0, removed: 0 };
      entry.added += file.added;
      entry.removed += file.removed;
      if (!entry.kind) {
        entry.kind = previewKindOfPath(file.path);
      }
      merged.set(key, entry);
    }
    return Array.from(merged.values()).sort((a, b) => a.key.localeCompare(b.key));
  }, [modifiedFiles, workdir]);

  const visibleModifiedFiles = filesExpanded ? mergedFiles : mergedFiles.slice(0, 6);
  const hiddenModifiedCount = mergedFiles.length - 6;

  return (
    <Panel
      id="modifiedFiles"
      title={panelBlock('modifiedFiles').title}
      icon={<FileIcon className="h-3.5 w-3.5" />}
      summary={mergedFiles.length > 0 ? mergedFiles.length : undefined}
    >
      {mergedFiles.length === 0 && <div className="text-xs text-muted">Aucun fichier modifié.</div>}
      {visibleModifiedFiles.map((file) => {
        const relative = workdir ? file.key : undefined;
        const kind = file.kind;
        const previewable = kind !== undefined && relative !== undefined && opener !== undefined;
        // A previewable file is a whole-row button: the accent wash is the only hover cue.
        const rowClass = `flex w-full items-center gap-2 py-1.5 font-mono text-xs text-left${
          previewable ? ' cursor-pointer rounded-md hover:bg-accent-soft' : ''
        }`;
        const pathClass = `grow overflow-hidden text-left text-ellipsis whitespace-nowrap${
          previewable ? ' group-hover:text-accent-strong' : ''
        }`;
        const row = (
          <>
            {/* Cut on the left: the file name is what tells files apart. */}
            <span dir="rtl" title={file.key} className={pathClass}>
              <bdi>{file.key}</bdi>
            </span>
            <span className="text-ok tabular-nums">+{file.added}</span>
            <span className="text-muted tabular-nums">−{file.removed}</span>
            {previewable && (
              <EyeIcon className="h-3.5 w-3.5 shrink-0 text-muted opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100" />
            )}
          </>
        );
        return previewable ? (
          <button
            key={file.key}
            type="button"
            className={`group ${rowClass}`}
            onClick={() => opener({ path: relative, kind })}
            title="Aperçu"
            aria-label={`Aperçu ${file.key}`}
          >
            {row}
          </button>
        ) : (
          <div key={file.key} className={rowClass}>
            {row}
          </div>
        );
      })}
      {hiddenModifiedCount > 0 && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setFilesExpanded((value) => !value)}
          className="self-start"
        >
          {filesExpanded ? 'Réduire' : `+ ${hiddenModifiedCount} autres`}
        </Button>
      )}
    </Panel>
  );
}
