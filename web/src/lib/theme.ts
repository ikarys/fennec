export type ThemeId = 'nuit' | 'jour' | 'brume';

export interface ThemeOption {
  id: ThemeId;
  label: string;
  swatch: string;
}

export const themes: ThemeOption[] = [
  { id: 'nuit', label: 'Nuit', swatch: '#0E1013' },
  { id: 'jour', label: 'Jour', swatch: '#F6F6F3' },
  { id: 'brume', label: 'Brume', swatch: '#0F1420' }
];

const STORAGE_KEY = 'fennec.theme';

export function applyTheme(id: ThemeId): void {
  document.documentElement.dataset.theme = id;
  try {
    localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // localStorage can be unavailable in private mode.
  }
}

export function getStoredTheme(): ThemeId {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    if (value === 'nuit' || value === 'jour' || value === 'brume') {
      return value;
    }
  } catch {
    // Ignore storage errors.
  }
  return 'nuit';
}
