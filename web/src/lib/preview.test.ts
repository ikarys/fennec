import { describe, expect, it } from 'vitest';
import { previewKindOfLanguage, previewKindOfPath } from './preview';

describe('previewKindOfPath', () => {
  it('maps known extensions case-insensitively', () => {
    expect(previewKindOfPath('a.HTML')).toBe('html');
    expect(previewKindOfPath('a.HTM')).toBe('html');
    expect(previewKindOfPath('a.MD')).toBe('markdown');
    expect(previewKindOfPath('a.Markdown')).toBe('markdown');
    expect(previewKindOfPath('a.SVG')).toBe('svg');
    expect(previewKindOfPath('a.MMD')).toBe('mermaid');
    expect(previewKindOfPath('a.Mermaid')).toBe('mermaid');
    expect(previewKindOfPath('a.JSON')).toBe('json');
    expect(previewKindOfPath('a.PNG')).toBe('image');
    expect(previewKindOfPath('a.JPG')).toBe('image');
    expect(previewKindOfPath('a.JPEG')).toBe('image');
    expect(previewKindOfPath('a.GIF')).toBe('image');
    expect(previewKindOfPath('a.WEBP')).toBe('image');
    expect(previewKindOfPath('a.PDF')).toBe('pdf');
  });

  it('returns undefined for unknown or missing extensions', () => {
    expect(previewKindOfPath('a.txt')).toBeUndefined();
    expect(previewKindOfPath('noext')).toBeUndefined();
  });
});

describe('previewKindOfLanguage', () => {
  it('maps block languages case-insensitively', () => {
    expect(previewKindOfLanguage('HTML')).toBe('html');
    expect(previewKindOfLanguage('markdown')).toBe('markdown');
    expect(previewKindOfLanguage('MD')).toBe('markdown');
    expect(previewKindOfLanguage('Svg')).toBe('svg');
    expect(previewKindOfLanguage('MERMAID')).toBe('mermaid');
    expect(previewKindOfLanguage('Json')).toBe('json');
  });

  it('returns undefined for non-block languages', () => {
    expect(previewKindOfLanguage(undefined)).toBeUndefined();
    expect(previewKindOfLanguage('pdf')).toBeUndefined();
  });
});
