/**
 * The version of the plugin blocks, from the `/api/notices` snapshot: it
 * moves on every accepted block update, so a component that fetches blocks
 * adds it to its fetch effect's deps and refetches — no polling. The
 * provider's value is set in App, from `useTurnNotices`.
 */
import { createContext, useContext } from 'react';

export const PluginsVersionContext = createContext(0);

export function usePluginsVersion(): number {
  return useContext(PluginsVersionContext);
}
