import { describe, expect, it } from 'vitest';
import { comboLabel, comboOf, shortcutIndex } from './shortcuts';

describe('shortcuts', () => {
  it('turns a key event into its combo', () => {
    expect(
      comboOf({ code: 'Digit1', altKey: true, shiftKey: true, ctrlKey: false, metaKey: false })
    ).toBe('Alt+Shift+Digit1');
    expect(
      comboOf({ code: 'KeyK', metaKey: true, altKey: false, ctrlKey: false, shiftKey: false })
    ).toBe('Mod+KeyK');
    expect(
      comboOf({ code: 'ShiftLeft', altKey: false, shiftKey: true, ctrlKey: false, metaKey: false })
    ).toBeNull();
  });

  it('renders combos in a human form', () => {
    expect(comboLabel('Alt+Shift+Digit0')).toBe('Alt+Shift+0');
    expect(comboLabel('Mod+Backslash')).toBe('Ctrl+\\');
    expect(comboLabel('Alt+Shift+ArrowRight')).toBe('Alt+Shift+→');
  });

  it('indexes only the non-empty combos', () => {
    const index = shortcutIndex([
      { action: 'newChat', combo: '', default: 'Mod+KeyN' },
      { action: 'palette', combo: 'Mod+KeyK', default: 'Mod+KeyK' }
    ]);
    expect(index.get('')).toBeUndefined();
    expect(index.get('Mod+KeyK')).toBe('palette');
  });
});
