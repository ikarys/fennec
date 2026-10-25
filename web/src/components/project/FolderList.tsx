import { useRef, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import type { Folder } from '../../lib/api';
import { FolderIcon, RepoIcon, SpinnerIcon } from '../Icons';

interface FolderListProps {
  folders: Folder[];
  parent: string | null;
  selected: string | null;
  loading: boolean;
  loadError: string | null;
  onSelect: (path: string) => void;
  onOpen: (path: string) => void;
}

/** The sub-folders of the current folder: click selects, double click or Enter opens. */
export function FolderList({ folders, parent, selected, loading, loadError, onSelect, onOpen }: FolderListProps) {
  const rowRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      if (folders.length === 0) return;
      event.preventDefault();

      const currentIndex = folders.findIndex((folder) => folder.path === selected);
      const nextIndex =
        event.key === 'ArrowDown'
          ? currentIndex === -1
            ? 0
            : Math.min(currentIndex + 1, folders.length - 1)
          : currentIndex === -1
            ? folders.length - 1
            : Math.max(currentIndex - 1, 0);

      const nextFolder = folders[nextIndex];
      if (!nextFolder) return;

      onSelect(nextFolder.path);
      rowRefs.current[nextIndex]?.focus();
    } else if (event.key === 'Backspace') {
      if (!parent) return;
      event.preventDefault();
      onOpen(parent);
    }
  };

  return (
    <div
      tabIndex={0}
      onKeyDown={handleKeyDown}
      aria-label="Liste des dossiers"
      className="min-h-0 flex-1 overflow-y-auto rounded-lg border border-border bg-raised p-2 outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      {loading ? (
        <div className="flex items-center justify-center py-8">
          <SpinnerIcon className="h-5 w-5 text-muted" />
        </div>
      ) : (
        <>
          {loadError && <div className="mb-2 text-sm text-err">{loadError}</div>}
          {folders.length === 0 ? (
            <p className="py-4 text-center text-sm text-muted">Aucun sous-dossier</p>
          ) : (
            <ul className="space-y-1">
              {folders.map((folder, index) => {
                const isSelected = selected === folder.path;
                return (
                  <li key={folder.path}>
                    <button
                      type="button"
                      ref={(element) => {
                        rowRefs.current[index] = element;
                      }}
                      onClick={() => onSelect(folder.path)}
                      onDoubleClick={() => onOpen(folder.path)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.preventDefault();
                          onOpen(folder.path);
                        }
                      }}
                      aria-pressed={isSelected}
                      className={`flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-sm ${
                        isSelected ? 'bg-accent-soft text-text' : 'text-text hover:bg-code'
                      }`}
                    >
                      {folder.repo ? (
                        <RepoIcon className="h-4 w-4 shrink-0 text-accent" />
                      ) : (
                        <FolderIcon className="h-4 w-4 shrink-0 text-muted" />
                      )}
                      <span className="min-w-0 flex-1 truncate">{folder.name}</span>
                      {folder.repo && (
                        <span className="shrink-0 rounded bg-accent-soft px-1.5 py-0.5 text-xs text-accent">
                          git
                        </span>
                      )}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </>
      )}
    </div>
  );
}
