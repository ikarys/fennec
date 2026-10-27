import { createContext, useContext } from 'react';

export interface PlanActions {
  segmentId: string;
  onBuild: () => void;
  onBuildAlone: () => void;
  onRevise: () => void;
  disabled?: boolean;
}

export const PlanActionsContext = createContext<PlanActions | null>(null);

export function usePlanActions(): PlanActions | null {
  return useContext(PlanActionsContext);
}
