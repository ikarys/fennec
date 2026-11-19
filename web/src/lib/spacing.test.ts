/// <reference types="vite/client" />
import { describe, it, expect } from 'vitest';
import { defaultDisplayPrefs } from './display';
import appCss from '../styles/app.css?raw';

// The Text size setting scales the root font size (App.tsx) and every text
// size is in rem. Spacing must follow it, so --spacing in styles/app.css is
// read back and checked in px at each root font size. `?raw` needs
// `test.css: true` (vite.config.ts): vitest otherwise stubs `.css` to empty.

function spacingRem(): number {
  const match = appCss.match(/^  --spacing: ([0-9]+(?:\.[0-9]+)?)rem;$/m);
  expect(match, 'styles/app.css: --spacing must be expressed in rem').not.toBeNull();
  return Number(match![1]);
}

// App.tsx: the root font size, in px, for a chosen Text size.
function rootFontPx(fontSize: number): number {
  return (16 * fontSize) / defaultDisplayPrefs.fontSize;
}

describe('--spacing', () => {
  it('is rem-based, so it scales with the root font size', () => {
    expect(spacingRem()).toBeGreaterThan(0);
  });

  it('computes to 3.5px at the default Text size, so the layout does not move', () => {
    expect(spacingRem() * rootFontPx(defaultDisplayPrefs.fontSize)).toBe(3.5);
  });

  it('grows with the text at Text size 16', () => {
    expect(spacingRem() * rootFontPx(16)).toBeCloseTo(3.5 * (16 / defaultDisplayPrefs.fontSize));
  });
});
