// Project page disk tab: measured session worktree usage and space freed on the human's click.
import { useCallback, useEffect, useRef, useState } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { CopyButton } from '../session/CopyButton';
import { EraserIcon, HardDriveIcon, RefreshIcon, SpinnerIcon, TrashIcon } from '../Icons';
import {
  cleanWorktree,
  getProjectWorktrees,
  removeOrphanWorktree,
  type ProjectWorktrees,
  type WorktreeEntry,
} from '../../lib/api/worktrees';
import { discardWorktree } from '../../lib/api';
import { formatBytes, formatRelative } from '../../lib/format';

export function DiskSection({
  projectId,
  onOpenSession,
}: {
  projectId: string;
  onOpenSession: (sessionId: string) => void;
}) {
  const [data, setData] = useState<ProjectWorktrees | null>(null);
  const [measuring, setMeasuring] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [confirm, setConfirm] = useState<WorktreeEntry | null>(null);
  const mountedRef = useRef(true);
  const projectIdRef = useRef(projectId);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    projectIdRef.current = projectId;
    let cancelled = false;

    setData(null);
    setLoadError(null);
    setMeasuring(true);

    getProjectWorktrees(projectId, true)
      .then((result) => {
        if (!cancelled) {
          setData(result);
          setMeasuring(false);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLoadError(error instanceof Error ? error.message : String(error));
          setMeasuring(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const refresh = useCallback(async () => {
    setMeasuring(true);
    setLoadError(null);

    try {
      const result = await getProjectWorktrees(projectId, true);
      if (mountedRef.current && projectIdRef.current === projectId) {
        setData(result);
      }
    } catch (error: unknown) {
      if (mountedRef.current && projectIdRef.current === projectId) {
        setLoadError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current && projectIdRef.current === projectId) {
        setMeasuring(false);
      }
    }
  }, [projectId]);

  const runAction = useCallback(
    async (name: string, action: () => Promise<unknown>) => {
      setBusy((prev) => ({ ...prev, [name]: true }));
      setErrors((prev) => {
        if (!(name in prev)) return prev;
        const next = { ...prev };
        delete next[name];
        return next;
      });

      try {
        await action();
        await refresh();
      } catch (error: unknown) {
        if (mountedRef.current && projectIdRef.current === projectId) {
          setErrors((prev) => ({
            ...prev,
            [name]: error instanceof Error ? error.message : String(error),
          }));
        }
      } finally {
        if (mountedRef.current && projectIdRef.current === projectId) {
          setBusy((prev) => ({ ...prev, [name]: false }));
        }
      }
    },
    [projectId, refresh],
  );

  const handleConfirm = useCallback(async () => {
    if (!confirm) return;

    const entry = confirm;
    setConfirm(null);

    await runAction(entry.name, async () => {
      if (entry.orphan) {
        await removeOrphanWorktree(entry.name);
      } else if (entry.sessionId) {
        await discardWorktree(entry.sessionId);
      }
    });
  }, [confirm, runAction]);

  const sessions = data?.entries.filter((entry) => !entry.orphan) ?? [];
  const orphans = data?.entries.filter((entry) => entry.orphan) ?? [];

  const confirmTitle = confirm?.orphan ? 'Supprimer ce dossier ?' : 'Supprimer le worktree ?';
  const confirmBody = confirm
    ? confirm.orphan
      ? `${confirm.name} et ce que git en garde (branche, références) sont supprimés.`
      : `Le dossier et la branche fennec/${confirm.name} sont supprimés, avec les commits de l'agent non intégrés. Le journal de la session reste.`
    : '';

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <Tile
          icon={<HardDriveIcon />}
          label="Espace disque"
          figure={data ? formatBytes(data.totalBytes) : '…'}
          detail={data ? `mesuré ${formatRelative(data.scannedAt)}` : "Mesure de l'espace disque…"}
          tone="info"
          active={data ? data.totalBytes > 0 : false}
        />
        <Tile
          icon={<EraserIcon />}
          label="Cache de build"
          figure={data ? formatBytes(data.generatedBytes) : '…'}
          detail="target, node_modules, .venv : rebâti par une compilation"
          tone="accent"
          active={data ? data.generatedBytes > 0 : false}
        />
      </div>

      {loadError ? <div className="px-4 pb-2 text-err text-xs">{loadError}</div> : null}

      <Section
        title="Worktrees des sessions"
        flush
        actions={
          <>
            {data ? <Badge tone="ok" pill title="Mesuré sur le disque">vérifié</Badge> : null}
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              title="Mesurer à nouveau"
              aria-label="Mesurer à nouveau"
              disabled={measuring}
              onClick={() => void refresh()}
            >
              {measuring ? <SpinnerIcon className="h-3.5 w-3.5" /> : <RefreshIcon className="h-3.5 w-3.5" />}
            </Button>
          </>
        }
      >
        {sessions.length === 0 ? (
          <Card className="px-4 py-6 text-center text-sm text-muted">Aucun worktree.</Card>
        ) : (
          <Card className="overflow-hidden">
            <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
              <span className="min-w-0 grow">Worktree</span>
              <span className="w-32 shrink-0 text-right">Taille</span>
              <span className="w-16 shrink-0" />
            </div>
            <ul className="divide-y divide-border">
              {sessions.map((entry) => (
                <SessionRow
                  key={entry.name}
                  entry={entry}
                  busy={Boolean(busy[entry.name])}
                  error={errors[entry.name]}
                  onOpenSession={onOpenSession}
                  onClean={() => void runAction(entry.name, () => cleanWorktree(entry.name))}
                  onTrash={() => setConfirm(entry)}
                />
              ))}
            </ul>
          </Card>
        )}
      </Section>

      {orphans.length > 0 ? (
        <Section
          title="Orphelins"
          description="Des dossiers qu'aucune session ou que git ne connaît plus : fennec ne les supprime jamais seul."
          flush
        >
          <Card className="overflow-hidden">
            <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
              <span className="min-w-0 grow">Dossier</span>
              <span className="w-32 shrink-0 text-right">Taille</span>
              <span className="w-16 shrink-0" />
            </div>
            <ul className="divide-y divide-border">
              {orphans.map((entry) => (
                <OrphanRow
                  key={entry.name}
                  entry={entry}
                  busy={Boolean(busy[entry.name])}
                  error={errors[entry.name]}
                  onTrash={() => setConfirm(entry)}
                />
              ))}
            </ul>
          </Card>
        </Section>
      ) : null}

      <ConfirmDialog
        open={confirm !== null}
        title={confirmTitle}
        body={confirmBody}
        confirmLabel="Supprimer"
        onConfirm={() => void handleConfirm()}
        onCancel={() => setConfirm(null)}
      />
    </div>
  );
}

function SessionRow({
  entry,
  busy,
  error,
  onOpenSession,
  onClean,
  onTrash,
}: {
  entry: WorktreeEntry;
  busy: boolean;
  error?: string;
  onOpenSession: (sessionId: string) => void;
  onClean: () => void;
  onTrash: () => void;
}) {
  return (
    <li className="group relative flex items-center gap-3 px-4 py-3 text-sm transition-colors before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:transition-opacity hover:bg-accent-soft hover:before:opacity-100 before:opacity-0">
      <div className="min-w-0 grow">
        <button
          type="button"
          className="text-left group-hover:text-accent-strong"
          onClick={() => {
            if (entry.sessionId) onOpenSession(entry.sessionId);
          }}
        >
          {entry.sessionTitle ? (
            <span>{entry.sessionTitle}</span>
          ) : entry.cache ? (
            <span>Cache Cargo partagé</span>
          ) : (
            <span className="font-mono text-xs">{entry.name}</span>
          )}
        </button>
        <div className="mt-1 flex items-center gap-2">
          {entry.updatedAt ? (
            <span className="text-xs text-muted">modifiée {formatRelative(entry.updatedAt)}</span>
          ) : null}
          {entry.running ? <Badge tone="accent" dot pulse>en cours</Badge> : null}
        </div>
        {error ? <p className="mt-1 text-err text-xs">{error}</p> : null}
      </div>

      <div className="w-32 shrink-0 text-right tabular-nums">
        <div>{formatBytes(entry.bytes)}</div>
        <div className="text-xs text-muted">dont {formatBytes(entry.generatedBytes)} de cache</div>
      </div>

      <div className="flex w-16 shrink-0 items-center justify-end gap-1">
        <Button
          variant="soft"
          size="sm"
          iconOnly
          title={`Libérer le cache de build (${formatBytes(entry.generatedBytes)})`}
          aria-label={`Libérer le cache de build (${formatBytes(entry.generatedBytes)})`}
          disabled={busy || entry.running || entry.generatedBytes === 0}
          onClick={onClean}
        >
          <EraserIcon className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="danger"
          size="sm"
          iconOnly
          title="Supprimer le worktree et sa branche"
          aria-label="Supprimer le worktree et sa branche"
          disabled={busy || entry.running}
          onClick={onTrash}
        >
          <TrashIcon className="h-3.5 w-3.5" />
        </Button>
      </div>
    </li>
  );
}

function OrphanRow({
  entry,
  busy,
  error,
  onTrash,
}: {
  entry: WorktreeEntry;
  busy: boolean;
  error?: string;
  onTrash: () => void;
}) {
  return (
    <li className="group relative flex items-center gap-3 px-4 py-3 text-sm transition-colors before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:transition-opacity hover:bg-accent-soft hover:before:opacity-100 before:opacity-0">
      <div className="min-w-0 grow">
        <div className="font-mono text-xs">{entry.name}</div>
        <div className="mt-1 text-xs text-muted">
          {entry.repoMissing ? 'dépôt introuvable' : 'inconnu de git ou sans session'}
        </div>
        {entry.sudoCommand ? (
          <div className="mt-2">
            <div className="flex items-center gap-2">
              <code className="rounded bg-code px-2 py-1 font-mono text-xs text-text">{entry.sudoCommand}</code>
              <CopyButton text={entry.sudoCommand} />
            </div>
            <p className="mt-1 text-xs text-muted">
              Des fichiers appartiennent à root : à lancer dans un terminal.
            </p>
          </div>
        ) : null}
        {error ? <p className="mt-1 text-err text-xs">{error}</p> : null}
      </div>

      <div className="w-32 shrink-0 text-right tabular-nums">{formatBytes(entry.bytes)}</div>

      <div className="flex w-16 shrink-0 items-center justify-end gap-1">
        {entry.sudoCommand ? null : (
          <Button
            variant="danger"
            size="sm"
            iconOnly
            title="Supprimer ce dossier"
            aria-label="Supprimer ce dossier"
            disabled={busy}
            onClick={onTrash}
          >
            <TrashIcon className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>
    </li>
  );
}
