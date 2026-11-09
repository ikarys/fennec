// localStorage helpers for the context rail's pinned state and open tab.
const PINNED_KEY = 'fennec.contextPanel.pinned';
const TAB_KEY = 'fennec.contextPanel.tab';

export function loadPinned(): boolean {
  try {
    return localStorage.getItem(PINNED_KEY) === '1';
  } catch {
    return false;
  }
}

export function loadTab(): string | null {
  try {
    return localStorage.getItem(TAB_KEY);
  } catch {
    return null;
  }
}

export function savePinned(pinned: boolean): void {
  try {
    localStorage.setItem(PINNED_KEY, pinned ? '1' : '0');
  } catch {
    // A blocked storage only loses the pin state on reload.
  }
}

export function saveTab(tab: string): void {
  try {
    localStorage.setItem(TAB_KEY, tab);
  } catch {
    // A blocked storage only loses the open tab on reload.
  }
}
