// Pure helpers for the editor page's open-file tabs.

/** The open list after a file was opened: appended at the end, never duplicated. */
export function openFileTab(open: string[], file: string): string[] {
  return open.includes(file) ? open : [...open, file];
}

/** Closes a tab: the remaining list and the tab the editor falls back to (its last remaining one, or null). */
export function closeFileTab(open: string[], file: string): { open: string[]; fallback: string | null } {
  const remaining = open.filter((path) => path !== file);
  return { open: remaining, fallback: remaining[remaining.length - 1] ?? null };
}
