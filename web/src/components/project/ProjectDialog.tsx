import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { createFolder, listFolders, type FolderListing } from '../../lib/api';
import { CloseIcon } from '../Icons';
import { FolderList } from './FolderList';
import { PathBreadcrumb } from './PathBreadcrumb';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { TextInput } from '../ui/Field';
import { Modal } from '../ui/Modal';

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
  const [newName, setNewName] = useState('');
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const requestRef = useRef(0);

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
    setNewName('');
    setCreateError(null);
    setCreating(false);
    setLoading(true);
    load();

    const timer = window.setTimeout(() => document.getElementById('project-dialog-path')?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [open, load]);

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

  const handleCreate = async () => {
    if (creating) return;
    const name = newName.trim();
    if (!currentPath || !name) return;
    setCreating(true);
    setCreateError(null);
    try {
      const created = await createFolder(currentPath, name);
      // onSubmit never throws (it owns its error state): awaiting it keeps
      // `creating` true through the open, so a failure there re-enables the
      // button instead of a stray double-create.
      await onSubmit(created.path);
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Impossible de créer le dossier');
    } finally {
      setCreating(false);
    }
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
    <Modal open={open} onClose={onClose} busy={submitting} label="Ouvrir un projet" width="w-[min(92vw,672px)]">
      <Card className="flex max-h-[85vh] w-full flex-col">
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-border p-4">
          <div className="flex min-w-0 flex-col gap-1">
            <h2 className="truncate text-base font-semibold text-text">Ouvrir un projet</h2>
            <p className="truncate text-sm text-muted">Choisissez le dossier du projet.</p>
          </div>
          <Button variant="secondary" size="sm" iconOnly onClick={onClose} aria-label="Fermer">
            <CloseIcon className="h-4 w-4" />
          </Button>
        </header>

        <div className="flex shrink-0 flex-col gap-3 border-b border-border p-4">
          <div className="flex items-center gap-2">
            <label htmlFor="project-dialog-path" className="sr-only">
              Chemin du dossier
            </label>
            <TextInput
              id="project-dialog-path"
              value={pathInput}
              onChange={(event) => setPathInput(event.target.value)}
              onKeyDown={handlePathKeyDown}
              placeholder="/"
              className="min-w-0 flex-1"
            />
            <Button
              variant="secondary"
              size="sm"
              iconOnly
              aria-label="Dossier parent"
              disabled={!listing?.parent}
              onClick={() => {
                if (listing?.parent) navigateTo(listing.parent);
              }}
            >
              ↑
            </Button>
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

          <div className="mt-3 flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <label htmlFor="project-dialog-new-name" className="shrink-0 text-sm text-muted">
                Nouveau projet
              </label>
              <TextInput
                id="project-dialog-new-name"
                value={newName}
                onChange={(event) => setNewName(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') handleCreate();
                }}
                placeholder="Nom du dossier"
                className="min-w-0 flex-1"
              />
              <Button
                variant="secondary"
                size="sm"
                disabled={creating || !currentPath || newName.trim().length === 0}
                onClick={handleCreate}
              >
                {creating ? 'Création…' : 'Créer et ouvrir'}
              </Button>
            </div>
            {createError && <div className="text-sm text-err">{createError}</div>}
          </div>
        </div>

        <footer className="flex shrink-0 flex-col gap-3 border-t border-border p-4">
          {error && <div className="text-sm text-err">{error}</div>}
          <div className="flex items-center gap-2 text-sm text-muted">
            <span className="shrink-0">Dossier :</span>
            <span className="min-w-0 truncate font-mono text-text" title={targetPath}>
              {targetPath || '—'}
            </span>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Button variant="secondary" size="sm" onClick={onClose}>
              Annuler
            </Button>
            <Button
              variant="primary"
              size="sm"
              disabled={submitting || !targetPath}
              onClick={() => {
                if (targetPath) onSubmit(targetPath);
              }}
            >
              {submitting ? 'Ouverture…' : 'Ouvrir ce dossier'}
            </Button>
          </div>
        </footer>
      </Card>
    </Modal>
  );
}
