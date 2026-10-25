import { useEffect, useRef, useState } from 'react';

interface ProjectDialogProps {
  open: boolean;
  submitting: boolean;
  error: string | null;
  onClose: () => void;
  onSubmit: (workdir: string) => void;
}

export function ProjectDialog({ open, submitting, error, onClose, onSubmit }: ProjectDialogProps) {
  const [workdir, setWorkdir] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setWorkdir('');
      window.setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  if (!open) return null;

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
      <form
        className="w-full max-w-md rounded-xl border border-border-strong bg-panel p-4 shadow-2xl"
        onSubmit={(event) => {
          event.preventDefault();
          if (workdir.trim()) onSubmit(workdir.trim());
        }}
      >
        <h2 className="text-base font-semibold">Ouvrir un projet</h2>
        <p className="mt-1 text-sm text-muted">Chemin absolu du dépôt local.</p>
        <label htmlFor="project-path" className="mt-4 block text-xs font-medium text-muted">
          Chemin du projet
        </label>
        <input
          id="project-path"
          ref={inputRef}
          value={workdir}
          onChange={(event) => setWorkdir(event.target.value)}
          placeholder="/home/user/workspace/fennec"
          className="mt-1 w-full rounded-lg border border-border bg-raised px-3 py-2 font-mono text-sm text-text"
        />
        {error && <div className="mt-2 text-sm text-err">{error}</div>}
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="min-h-9 rounded-lg border border-border bg-transparent px-3 text-sm text-muted"
          >
            Annuler
          </button>
          <button
            type="submit"
            disabled={submitting || !workdir.trim()}
            className="min-h-9 rounded-lg bg-accent px-3 text-sm font-medium text-on-accent disabled:opacity-60"
          >
            {submitting ? 'Ouverture…' : 'Ouvrir'}
          </button>
        </div>
      </form>
    </div>
  );
}
