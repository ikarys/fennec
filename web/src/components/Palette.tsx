import { useEffect, useMemo, useState } from 'react';
import type { Project } from '../lib/api';
import { stateDotClass, stateLabel } from '../lib/format';
import { SearchIcon } from './Icons';

interface PaletteItem {
  id: string;
  title: string;
  meta: string;
  dot: string;
  action: () => void;
}

interface PaletteGroup {
  label: string;
  items: PaletteItem[];
}

interface PaletteProps {
  open: boolean;
  projects: Project[];
  onClose: () => void;
  onOpenSession: (sessionId: string) => void;
  onNewSession: (projectId: string) => void;
  onOpenProjectDialog: () => void;
  onHome: () => void;
}

export function Palette({ open, projects, onClose, onOpenSession, onNewSession, onOpenProjectDialog, onHome }: PaletteProps) {
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);

  const groups = useMemo<PaletteGroup[]>(() => {
    const q = query.trim().toLowerCase();
    const allSessions = projects.flatMap((project) =>
      project.sessions.map((session) => ({ ...session, project: project.name, projectId: project.id }))
    );

    const matchesSession = (session: (typeof allSessions)[number]) =>
      !q || session.title.toLowerCase().includes(q) || session.project.toLowerCase().includes(q);

    const recent = [...allSessions].sort((a, b) => b.updatedAt - a.updatedAt).slice(0, 5);

    const sessionItem = (session: (typeof allSessions)[number]): PaletteItem => ({
      id: `session-${session.id}`,
      title: session.title,
      meta: `${session.project} · ${stateLabel(session.state)}`,
      dot: stateDotClass(session.state),
      action: () => onOpenSession(session.id)
    });

    const result: PaletteGroup[] = [];

    const recentItems = recent.filter(matchesSession).map(sessionItem);
    if (recentItems.length > 0) {
      result.push({ label: 'Récentes', items: recentItems });
    }

    // Without a query the recent list is enough; per-project groups would
    // repeat it.
    for (const project of q ? projects : []) {
      const items = project.sessions
        .map((session) => ({ ...session, project: project.name, projectId: project.id }))
        .filter(matchesSession)
        .map(sessionItem);
      if (items.length > 0) {
        result.push({ label: project.name, items });
      }
    }

    const commands: PaletteItem[] = [
      ...projects.map((project) => ({
        id: `new-${project.id}`,
        title: `Nouvelle session dans ${project.name}`,
        meta: '⌘N',
        dot: 'bg-muted',
        action: () => onNewSession(project.id)
      })),
      {
        id: 'open-project',
        title: 'Ouvrir un projet…',
        meta: '',
        dot: 'bg-muted',
        action: onOpenProjectDialog
      },
      {
        id: 'home',
        title: 'Accueil',
        meta: '',
        dot: 'bg-muted',
        action: onHome
      }
    ].filter((command) => !q || command.title.toLowerCase().includes(q));

    if (commands.length > 0) {
      result.push({ label: 'Commandes', items: commands });
    }

    return result;
  }, [query, projects, onOpenSession, onNewSession, onOpenProjectDialog]);

  const flatItems = useMemo(() => groups.flatMap((group) => group.items), [groups]);

  useEffect(() => {
    if (open) {
      setQuery('');
      setActiveIndex(0);
    }
  }, [open]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query]);

  useEffect(() => {
    const element = document.querySelector('[data-palette-active="true"]');
    element?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex justify-center bg-scrim p-4 pt-[12vh]"
      role="dialog"
      aria-modal="true"
      aria-label="Palette de sessions"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-[640px] self-start overflow-hidden rounded-[14px] border border-border-strong bg-panel shadow-2xl">
        <div className="flex items-center gap-2.5 border-b border-border px-3.5">
          <SearchIcon className="h-4 w-4 text-muted" />
          <label htmlFor="palette-q" className="absolute h-px w-px overflow-hidden clip-[rect(0_0_0_0)]">
            Rechercher
          </label>
          <input
            id="palette-q"
            autoFocus
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setActiveIndex((index) => Math.min(flatItems.length - 1, index + 1));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setActiveIndex((index) => Math.max(0, index - 1));
              } else if (event.key === 'Enter') {
                event.preventDefault();
                const item = flatItems[activeIndex];
                if (item) {
                  item.action();
                  onClose();
                }
              } else if (event.key === 'Escape') {
                onClose();
              }
            }}
            placeholder="Rechercher une session, un projet, une commande…"
            className="grow min-h-[50px] bg-transparent text-[15px] outline-none"
          />
          <button
            type="button"
            onClick={onClose}
            aria-label="Fermer la palette"
            className="rounded-md border border-border px-2 py-0.5 text-[11px] text-muted"
          >
            Échap
          </button>
        </div>

        <div className="max-h-[420px] overflow-y-auto p-1.5">
          {flatItems.length === 0 && <div className="px-2.5 py-6 text-center text-sm text-muted">Aucun résultat</div>}
          {groups.map((group) => (
            <div key={group.label}>
              <div className="px-2.5 pt-2.5 pb-1 text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">
                {group.label}
              </div>
              {group.items.map((item) => {
                const index = flatItems.findIndex((candidate) => candidate.id === item.id);
                const active = index === activeIndex;
                return (
                  <button
                    key={item.id}
                    type="button"
                    data-palette-active={active}
                    onClick={() => {
                      item.action();
                      onClose();
                    }}
                    onMouseEnter={() => setActiveIndex(index)}
                    className={`flex w-full items-center gap-2.5 rounded-lg px-2.5 text-left ${
                      active ? 'bg-raised' : 'bg-transparent'
                    } min-h-10`}
                  >
                    <span className={`h-[7px] w-[7px] flex-none rounded-full ${item.dot}`} />
                    <span className="min-w-0 grow overflow-hidden text-ellipsis whitespace-nowrap font-medium">
                      {item.title}
                    </span>
                    <span className="whitespace-nowrap text-xs text-muted">{item.meta}</span>
                  </button>
                );
              })}
            </div>
          ))}
        </div>

        <div className="flex flex-wrap gap-4 border-t border-border px-3.5 py-2 text-xs text-muted">
          <span>↑↓ naviguer</span>
          <span>↵ ouvrir dans un onglet</span>
          <span>⌘↵ nouvelle session ici</span>
        </div>
      </div>
    </div>
  );
}
