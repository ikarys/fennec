import { createContext, useContext } from 'react';

export interface IntegrationInvite {
  /** The id of the thread's last workflow-end item: only that end card invites. */
  lastRunEndId: string | null;
  open: () => void;
}

export const IntegrationInviteContext = createContext<IntegrationInvite | null>(null);

export function useIntegrationInvite(): IntegrationInvite | null {
  return useContext(IntegrationInviteContext);
}
