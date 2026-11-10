// The keyboard shortcuts: the server resolves the bindings, a key press only looks its combo up and runs the action.
import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { Project } from '../lib/api';
import { PAGE_TABS, tabIdForRoute, type Route } from '../lib/route';
import { AGENT_SHORTCUT_EVENT, comboLabel, comboOf, shortcutIndex, type ShortcutAction, type ShortcutBinding } from '../lib/shortcuts';

/** The page each page shortcut opens. */
const SHORTCUT_PAGES = {
  goProjects: 'projects',
  goChats: 'chats',
  goTasks: 'backlog',
  goWorkflows: 'workflows',
  goInsight: 'insight',
  goSettings: 'settings'
} as const;

const AGENT_SHORTCUTS = { agentPlan: 'plan', agentBuilder: 'builder', agentAsk: 'ask' } as const;

type ShortcutContext = {
  bindings: ShortcutBinding[] | undefined;
  route: Route;
  projects: Project[];
  tabs: string[];
  chatProjectId: string | null;
  navigate: (path: string) => void;
  pickTab: (id: string) => void;
  closeTab: (id: string) => void;
  startNewSession: (projectId?: string) => void;
  togglePalette: () => void;
  toggleSplit: () => void;
};

/** Binds the shortcuts; returns the label of each bound action, for the palette. */
export function useShortcuts(context: ShortcutContext) {
  const { bindings, route, projects, tabs, chatProjectId } = context;
  const { navigate, pickTab, closeTab, startNewSession, togglePalette, toggleSplit } = context;
  const shortcuts = useMemo(() => shortcutIndex(bindings ?? []), [bindings]);
  const shortcutLabels = useMemo(
    () =>
      Object.fromEntries(
        (bindings ?? []).filter((binding) => binding.combo).map((binding) => [binding.action, comboLabel(binding.combo)])
      ) as Partial<Record<ShortcutAction, string>>,
    [bindings]
  );
  const runShortcut = useCallback(
    (action: ShortcutAction) => {
      switch (action) {
        case 'newChat':
          if (chatProjectId) startNewSession(chatProjectId);
          break;
        case 'agentPlan':
        case 'agentBuilder':
        case 'agentAsk':
          window.dispatchEvent(new CustomEvent(AGENT_SHORTCUT_EVENT, { detail: AGENT_SHORTCUTS[action] }));
          break;
        case 'newSession': {
          let projectId: string | undefined;
          if (route.name === 'new') {
            projectId = route.id;
          } else if (route.name === 'session') {
            const project = projects.find((candidate) => candidate.sessions.some((session) => session.id === route.id));
            projectId = project?.id;
          } else if (route.name === 'project') {
            projectId = route.projectId;
          }
          startNewSession(projectId);
          break;
        }
        case 'goHome':
          navigate('#/');
          break;
        case 'goProjects':
        case 'goChats':
        case 'goTasks':
        case 'goWorkflows':
        case 'goInsight':
        case 'goSettings':
          navigate(PAGE_TABS.find((tab) => tab.page === SHORTCUT_PAGES[action])?.path ?? '#/');
          break;
        case 'nextTab':
        case 'prevTab': {
          if (tabs.length === 0) break;
          const step = action === 'nextTab' ? 1 : -1;
          const index = tabs.indexOf(tabIdForRoute(route) ?? '');
          const target =
            index === -1
              ? step > 0
                ? tabs[0]
                : tabs[tabs.length - 1]
              : tabs[(index + step + tabs.length) % tabs.length];
          if (target) pickTab(target);
          break;
        }
        case 'closeTab': {
          const id = tabIdForRoute(route);
          if (id) closeTab(id);
          break;
        }
        case 'palette':
          togglePalette();
          break;
        case 'split':
          toggleSplit();
          break;
      }
    },
    [chatProjectId, closeTab, navigate, pickTab, projects, route, startNewSession, tabs, toggleSplit, togglePalette]
  );
  // The key handler is attached once per binding set; it always calls the latest action runner.
  const runShortcutRef = useRef(runShortcut);
  useEffect(() => {
    runShortcutRef.current = runShortcut;
  }, [runShortcut]);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat) return;
      const combo = comboOf(event);
      const action = combo ? shortcuts.get(combo) : undefined;
      if (!action) return;
      event.preventDefault();
      runShortcutRef.current(action);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [shortcuts]);

  return shortcutLabels;
}
