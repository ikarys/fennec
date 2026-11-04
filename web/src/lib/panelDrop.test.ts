import { describe, expect, it } from 'vitest';

import type { PanelPage } from './api';
import { dropBlock } from './panelBlocks';

function makePages(): PanelPage[] {
  return [
    { name: 'A', blocks: ['servers', 'skills', 'tokens'] },
    { name: 'B', blocks: ['vision'] }
  ];
}

function makeDuplicateTargetPages(): PanelPage[] {
  return [
    { name: 'A', blocks: ['servers', 'skills', 'tokens'] },
    { name: 'B', blocks: ['tokens', 'vision'] }
  ];
}

describe('dropBlock', () => {
  it('reorders a block down within the same page', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'servers', page: 0 }, 0, 2);

    expect(result).not.toBe(pages);
    expect(result[0]?.blocks).toEqual(['skills', 'servers', 'tokens']);
    expect(result[1]?.blocks).toEqual(['vision']);
  });

  it('reorders a block up within the same page', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'tokens', page: 0 }, 0, 0);

    expect(result[0]?.blocks).toEqual(['tokens', 'servers', 'skills']);
  });

  it('returns the same reference for a no-op drop at the block position', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'servers', page: 0 }, 0, 0);

    expect(result).toBe(pages);
  });

  it('returns the same reference for a no-op drop right after the block position', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'servers', page: 0 }, 0, 1);

    expect(result).toBe(pages);
  });

  it('moves a block from one page to another', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'servers', page: 0 }, 1, 0);

    expect(result[0]?.blocks).toEqual(['skills', 'tokens']);
    expect(result[1]?.blocks).toEqual(['servers', 'vision']);
  });

  it('moves an existing target block and removes it from the source page', () => {
    const pages = makeDuplicateTargetPages();
    const result = dropBlock(pages, { id: 'tokens', page: 0 }, 1, 2);

    expect(result[0]?.blocks).toEqual(['servers', 'skills']);
    expect(result[1]?.blocks).toEqual(['vision', 'tokens']);
  });

  it('removes from the source page when a cross-page duplicate target move is a no-op', () => {
    const pages = makeDuplicateTargetPages();
    const result = dropBlock(pages, { id: 'tokens', page: 0 }, 1, 1);

    expect(result[0]?.blocks).toEqual(['servers', 'skills']);
    expect(result[1]?.blocks).toEqual(['tokens', 'vision']);
  });

  it('adds a block from the available chips', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'plugins', page: null }, 0, 1);

    expect(result[0]?.blocks).toEqual(['servers', 'plugins', 'skills', 'tokens']);
    expect(result[1]?.blocks).toEqual(['vision']);
  });

  it('treats a chip drop as a same-page move when the page already has the block', () => {
    const pages = makePages();
    const result = dropBlock(pages, { id: 'servers', page: null }, 0, 2);

    expect(result[0]?.blocks).toEqual(['skills', 'servers', 'tokens']);
  });

  it('clamps negative and oversized toIndex values', () => {
    const pages = makePages();
    const low = dropBlock(pages, { id: 'plugins', page: null }, 0, -5);
    expect(low[0]?.blocks).toEqual(['plugins', 'servers', 'skills', 'tokens']);

    const pages2 = makePages();
    const high = dropBlock(pages2, { id: 'plugins', page: null }, 0, 99);
    expect(high[0]?.blocks).toEqual(['servers', 'skills', 'tokens', 'plugins']);

    const pages3 = makePages();
    const clampedSamePage = dropBlock(pages3, { id: 'servers', page: 0 }, 0, 99);
    expect(clampedSamePage[0]?.blocks).toEqual(['skills', 'tokens', 'servers']);
  });

  it('returns unchanged pages for out-of-range pages', () => {
    const pages = makePages();

    expect(dropBlock(pages, { id: 'servers', page: 0 }, 2, 0)).toBe(pages);
    expect(dropBlock(pages, { id: 'servers', page: -1 }, 0, 0)).toBe(pages);
    expect(dropBlock(pages, { id: 'servers', page: 5 }, 0, 0)).toBe(pages);
  });

  it('does not mutate the input pages', () => {
    const pages = makePages();
    const before = JSON.parse(JSON.stringify(pages)) as PanelPage[];

    const result = dropBlock(pages, { id: 'servers', page: 0 }, 1, 0);

    expect(pages).toEqual(before);
    expect(result).not.toBe(pages);
    expect(result[0]).not.toBe(pages[0]);
    expect(result[1]).not.toBe(pages[1]);
    expect(result[0]?.blocks).not.toBe(pages[0]?.blocks);
    expect(result[1]?.blocks).not.toBe(pages[1]?.blocks);
  });
});
