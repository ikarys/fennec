import { readStored, writeStored } from './storage';

export type ThemeId = 'nuit' | 'jour' | 'ardoise' | 'cyber';

export interface ThemeOption {
  id: ThemeId;
  label: string;
}

export const themes: ThemeOption[] = [
  { id: 'nuit', label: 'Nuit' },
  { id: 'jour', label: 'Jour' },
  { id: 'ardoise', label: 'Ardoise' },
  { id: 'cyber', label: 'Cyber' }
];

const STORAGE_KEY = 'fennec.theme';

export function applyTheme(id: ThemeId): void {
  document.documentElement.dataset.theme = id;
  writeStored(STORAGE_KEY, id);
}

export function getStoredTheme(): ThemeId {
  const value = readStored(STORAGE_KEY);
  if (value === 'nuit' || value === 'jour' || value === 'ardoise' || value === 'cyber') {
    return value;
  }
  return 'nuit';
}
