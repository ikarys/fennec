import { createContext, useContext } from 'react';

export interface PlanActions {
  segmentId: string;
  onBuild: (worktree: boolean) => void;
  onBuildAlone: () => void;
  onRevise: () => void;
  /** Set when the session frames an idea: the plan becomes the idea's, ready. */
  ideaReady?: { onMark: () => void; already: boolean };
  disabled?: boolean;
}

export const PlanActionsContext = createContext<PlanActions | null>(null);

export function usePlanActions(): PlanActions | null {
  return useContext(PlanActionsContext);
}
