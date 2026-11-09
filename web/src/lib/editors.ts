// The IDEs the open-in-editor links can target, and their tooltip.
import { useT } from '../i18n';
import { useDisplayPrefs } from './display';
import type { EditorId } from './api/settings';

export const EDITORS: { value: EditorId; label: string }[] = [
  { value: 'vscode', label: 'VS Code' },
  { value: 'vscode-insiders', label: 'VS Code Insiders' },
  { value: 'cursor', label: 'Cursor' },
  { value: 'windsurf', label: 'Windsurf' },
  { value: 'vscodium', label: 'VSCodium' },
];

export function editorLabel(id: EditorId | undefined): string {
  return EDITORS.find((editor) => editor.value === id)?.label ?? 'VS Code';
}

/** The tooltip of an open-in-editor link, naming the IDE chosen in Settings. */
export function useOpenInEditorLabel(key: 'openInIde' | 'openInEditor' = 'openInIde'): string {
  return useT()(key, { editor: editorLabel(useDisplayPrefs().editor) });
}
