import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { listFolders, type FolderListing } from '../../lib/api';
import { CloseIcon } from '../Icons';
import { FolderList } from './FolderList';
import { PathBreadcrumb } from './PathBreadcrumb';

interface ProjectDialogProps {
  open: boolean;
  submitting: boolean;
  error: string | null;
  onClose: () => void;
  onSubmit: (workdir: string) => void;
}

export function ProjectDialog({ open, submitting, error, onClose, onSubmit }: ProjectDialogProps) {
  const [listing, setListing] = useState<FolderListing | null>(null);
  const [pathInput, setPathInput] = useState('');
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [showHidden, setShowHidden] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const requestRef = useRef(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const load = useCallback(async (target?: string) => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setLoadError(null);

    try {
      const result = await listFolders(target);
      if (requestId !== requestRef.current) return;

      setListing(result);
      setPathInput(result.path);
      setSelectedPath(null);
    } catch (err) {
      if (requestId !== requestRef.current) return;
      setLoadError(err instanceof Error ? err.message : 'Impossible de lire le dossier');
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;

    setListing(null);
    setPathInput('');
    setSelectedPath(null);
    setShowHidden(false);
    setLoadError(null);
    setLoading(true);
    load();

    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [open, load]);

  useEffect(() => {
    if (!open) return;

    const handleWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };

    window.addEventListener('keydown', handleWindowKeyDown);
    return () => window.removeEventListener('keydown', handleWindowKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  const visibleFolders = listing
    ? showHidden
      ? listing.folders
      : listing.folders.filter((folder) => !folder.hidden)
    : [];
  const targetPath = selectedPath ?? listing?.path ?? pathInput;
  const currentPath = listing?.path ?? '';

  const navigateTo = (path: string) => {
    load(path);
  };

  const handlePathKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();

    const trimmed = pathInput.trim();
    load(trimmed ? trimmed : undefined);
  };

  const handleShowHiddenChange = (event: ChangeEvent<HTMLInputElement>) => {
    const checked = event.target.checked;
    setShowHidden(checked);

    if (!checked && selectedPath && listing) {
      const selectedFolder = listing.folders.find((folder) => folder.path === selectedPath);
      if (selectedFolder?.hidden) setSelectedPath(null);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Ouvrir un projet"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-border-strong bg-panel shadow-2xl">
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-border p-4">
          <div className="min-w-0">
            <h2 className="truncate text-base font-semibold text-text">Ouvrir un projet</h2>
            <p className="mt-1 truncate text-sm text-muted">Choisissez le dossier du projet.</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Fermer"
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border bg-raised text-muted hover:text-text"
          >
            <CloseIcon className="h-4 w-4" />
          </button>
        </header>

        <div className="shrink-0 space-y-3 border-b border-border p-4">
          <div className="flex items-center gap-2">
            <label htmlFor="project-dialog-path" className="sr-only">
              Chemin du dossier
            </label>
            <input
              id="project-dialog-path"
              ref={inputRef}
              value={pathInput}
              onChange={(event) => setPathInput(event.target.value)}
              onKeyDown={handlePathKeyDown}
              placeholder="/"
              className="min-w-0 flex-1 rounded-lg border border-border bg-code px-3 py-2 font-mono text-sm text-text"
            />
            <button
              type="button"
              aria-label="Dossier parent"
              disabled={!listing?.parent}
              onClick={() => {
                if (listing?.parent) navigateTo(listing.parent);
              }}
              className="min-h-9 shrink-0 rounded-lg border border-border bg-raised px-3 text-sm text-text disabled:opacity-50"
            >
              ↑
            </button>
          </div>

          {currentPath && <PathBreadcrumb path={currentPath} onNavigate={navigateTo} />}

          <label className="flex items-center gap-2 text-sm text-muted">
            <input
              type="checkbox"
              checked={showHidden}
              onChange={handleShowHiddenChange}
              className="h-4 w-4 rounded border-border bg-raised text-accent"
            />
            Afficher les dossiers cachés
          </label>
        </div>

        <div className="flex min-h-0 flex-1 flex-col p-4">
          <FolderList
            folders={visibleFolders}
            parent={listing?.parent ?? null}
            selected={selectedPath}
            loading={loading}
            loadError={loadError}
            onSelect={setSelectedPath}
            onOpen={navigateTo}
          />
        </div>

        <footer className="shrink-0 border-t border-border p-4">
          {error && <div className="mb-2 text-sm text-err">{error}</div>}
          <div className="mb-3 flex items-center gap-2 text-sm text-muted">
            <span className="shrink-0">Dossier :</span>
            <span className="min-w-0 truncate font-mono text-text" title={targetPath}>
              {targetPath || '—'}
            </span>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              className="min-h-9 rounded-lg border border-border bg-transparent px-3 text-sm text-muted"
            >
              Annuler
            </button>
            <button
              type="button"
              disabled={submitting || !targetPath}
              onClick={() => {
                if (targetPath) onSubmit(targetPath);
              }}
              className="min-h-9 rounded-lg bg-accent px-3 text-sm font-medium text-on-accent disabled:opacity-60"
            >
              {submitting ? 'Ouverture…' : 'Ouvrir ce dossier'}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
