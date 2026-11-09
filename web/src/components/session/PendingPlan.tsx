import { createContext, useContext } from 'react';

/** The call id of the plan waiting for the human's decision (served `pendingPlan`): its thread card shows it pending. */
export const PendingPlanContext = createContext<string | null>(null);

export function usePendingPlan(): string | null {
  return useContext(PendingPlanContext);
}

/** The DOM id of a plan's card in the thread, so the decision bar can scroll to it. */
export function planAnchorId(callId: string): string {
  return `plan-${callId}`;
}
