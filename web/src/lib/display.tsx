import { createContext, useContext } from 'react';
import type { ReactNode } from 'react';
import type { DisplayPrefs } from './api';

// Matches the server's defaults, used until the settings are loaded.
export const defaultDisplayPrefs: DisplayPrefs = {
  reasoningOpen: true,
  toolResultsOpen: false,
  fontSize: 14,
  panelPages: null
};

const DisplayPrefsContext = createContext<DisplayPrefs>(defaultDisplayPrefs);

export function DisplayPrefsProvider({ value, children }: { value: DisplayPrefs; children: ReactNode }) {
  return <DisplayPrefsContext.Provider value={value}>{children}</DisplayPrefsContext.Provider>;
}

export function useDisplayPrefs(): DisplayPrefs {
  return useContext(DisplayPrefsContext);
}
