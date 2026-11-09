import { createContext, useContext } from 'react';

export interface IntegrationInvite {
  open: () => void;
}

export const IntegrationInviteContext = createContext<IntegrationInvite | null>(null);

export function useIntegrationInvite(): IntegrationInvite | null {
  return useContext(IntegrationInviteContext);
}
