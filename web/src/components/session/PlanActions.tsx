import { createContext, useContext } from 'react';

export interface PlanActions {
  segmentId: string;
  onBuild: (worktree: boolean) => void;
  onBuildAlone: () => void;
  onRevise: () => void;
  /** The plan joins the backlog: a new idea (`none`), or the session's idea, ready;
   * `saved` once this very plan is there. */
  ideaReady?: { onMark: () => void; idea: 'none' | 'framing' | 'ready' | 'saved' };
  disabled?: boolean;
}

export const PlanActionsContext = createContext<PlanActions | null>(null);

export function usePlanActions(): PlanActions | null {
  return useContext(PlanActionsContext);
}
