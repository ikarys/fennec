// Keyboard shortcut combo parsing, labels, and the server binding catalog.
export type ShortcutAction =
  | 'newChat'
  | 'agentPlan'
  | 'agentBuilder'
  | 'agentAsk'
  | 'newSession'
  | 'goHome'
  | 'goProjects'
  | 'goChats'
  | 'goTasks'
  | 'goWorkflows'
  | 'goInsight'
  | 'goSettings'
  | 'nextTab'
  | 'prevTab'
  | 'closeTab'
  | 'palette'
  | 'split';

/** The window event a session page listens to: switch its composer's agent (the detail is the agent id). */
export const AGENT_SHORTCUT_EVENT = 'fennec:agent';

/** `comboOf`: the combo string of a key event, or null for a lone modifier key. */
export function comboOf(event: Pick<KeyboardEvent, 'code' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>): string | null {
  const code = event.code;
  if (!code) return null;
  if (
    code.startsWith('Control') ||
    code.startsWith('Alt') ||
    code.startsWith('Shift') ||
    code.startsWith('Meta') ||
    code.startsWith('OS')
  ) {
    return null;
  }
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push('Mod');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  parts.push(code);
  return parts.join('+');
}

/** How a combo reads: `Alt+Shift+Digit1` → `Alt+Shift+1`. */
export function comboLabel(combo: string): string {
  if (!combo) return '';
  return combo
    .split('+')
    .map((part) => {
      switch (part) {
        case 'Mod':
          return 'Ctrl';
        case 'ArrowLeft':
          return '←';
        case 'ArrowRight':
          return '→';
        case 'ArrowUp':
          return '↑';
        case 'ArrowDown':
          return '↓';
        case 'Backslash':
          return '\\';
        case 'Slash':
          return '/';
        case 'Comma':
          return ',';
        case 'Period':
          return '.';
        case 'Semicolon':
          return ';';
        case 'Quote':
          return "'";
        case 'BracketLeft':
          return '[';
        case 'BracketRight':
          return ']';
        case 'Minus':
          return '-';
        case 'Equal':
          return '=';
        case 'Backquote':
          return '`';
        default: {
          const digit = part.match(/^Digit([0-9])$/);
          if (digit && digit[1]) return digit[1];
          const key = part.match(/^Key([A-Z])$/);
          if (key && key[1]) return key[1];
          return part;
        }
      }
    })
    .join('+');
}

export interface ShortcutBinding {
  action: ShortcutAction;
  combo: string;
  default: string;
}

/** The action bound to each non-empty combo. */
export function shortcutIndex(bindings: ShortcutBinding[]): Map<string, ShortcutAction> {
  const index = new Map<string, ShortcutAction>();
  for (const binding of bindings) {
    if (binding.combo) {
      index.set(binding.combo, binding.action);
    }
  }
  return index;
}
