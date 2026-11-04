/**
 * The context panel's building blocks.
 *
 * A block is one bounded section of the right-hand panel; a page is a named
 * list of optional block ids. A block is `core` when it carries the
 * integration decision — the run's contract (criteria), what will be
 * integrated (modified files), the agent's workspace, and the request that
 * framed it. The core blocks open the first page and cannot be removed from
 * it; every other block is optional and the human places it on any page. The
 * non-core pages show only their own blocks, so they stay light.
 */
import type { PanelPage } from './api';
import { t, type Lang, type MsgKey } from '../i18n/messages';

export type PanelBlockId = 'request' | 'criteria' | 'modifiedFiles' | 'workspace' | 'servers' | 'tokens' | 'skills' | 'decisions' | 'vision' | 'plugins';

export interface PanelBlock {
  id: PanelBlockId;
  titleKey: MsgKey;
  core: boolean;
}

/** In display order: core blocks first. */
export const PANEL_BLOCKS: readonly PanelBlock[] = [
  { id: 'request', titleKey: 'panelRequest', core: true },
  { id: 'criteria', titleKey: 'panelCriteria', core: true },
  { id: 'modifiedFiles', titleKey: 'panelModifiedFiles', core: true },
  { id: 'workspace', titleKey: 'panelWorkspace', core: true },
  { id: 'servers', titleKey: 'panelServers', core: false },
  { id: 'skills', titleKey: 'panelSkills', core: false },
  { id: 'decisions', titleKey: 'panelDecisions', core: false },
  { id: 'tokens', titleKey: 'panelTokens', core: false },
  { id: 'vision', titleKey: 'panelVision', core: false },
  { id: 'plugins', titleKey: 'panelPlugins', core: false }
];

export const OPTIONAL_BLOCKS: readonly PanelBlock[] = PANEL_BLOCKS.filter((block) => !block.core);

/** The default pages, named in the UI's language; stored pages keep their names. */
export function defaultPanelPages(lang: Lang): PanelPage[] {
  return [
    { name: t(lang, 'panelPageDecision'), blocks: [] },
    { name: t(lang, 'panelPageContext'), blocks: ['skills', 'decisions', 'servers', 'plugins'] },
    { name: t(lang, 'panelPageStats'), blocks: ['tokens', 'vision'] }
  ];
}

/** The pages to show: the stored ones, or the defaults when none are stored. */
export function panelPages(lang: Lang, stored: PanelPage[] | null | undefined): PanelPage[] {
  return stored ?? defaultPanelPages(lang);
}

function isOptionalBlockId(value: string): value is PanelBlockId {
  return OPTIONAL_BLOCKS.some((block) => block.id === value);
}

/**
 * What a page shows: the core blocks on the default (first) page, then the
 * page's known optional blocks in its order, once each (unknown or core ids
 * in a stored page are ignored).
 */
export function pageBlocks(page: PanelPage | undefined, isDefault: boolean): PanelBlockId[] {
  const seen = new Set<PanelBlockId>();
  const result: PanelBlockId[] = isDefault ? PANEL_BLOCKS.filter((block) => block.core).map((block) => block.id) : [];

  for (const id of page?.blocks ?? []) {
    if (isOptionalBlockId(id) && !seen.has(id)) {
      seen.add(id);
      result.push(id);
    }
  }

  return result;
}

export function panelBlock(id: PanelBlockId): PanelBlock {
  const block = PANEL_BLOCKS.find((candidate) => candidate.id === id);
  if (!block) {
    throw new Error(`Unknown panel block: ${id}`);
  }
  return block;
}

/** A page's optional blocks as the panel shows them: known ids, once each, in its order. */
export function pageOptionalBlocks(page: PanelPage): PanelBlockId[] {
  return pageBlocks(page, false);
}

/** A block being dragged: from a page, or from the chips of blocks a page does not show (`page: null`). */
export interface BlockDrag {
  id: PanelBlockId;
  page: number | null;
}

/**
 * Drops a dragged block into a page before the block at `toIndex` (the end when past it).
 * From another page it moves, from the chips it is added; a drop where it already is changes nothing.
 */
export function dropBlock(pages: PanelPage[], drag: BlockDrag, toPage: number, toIndex: number): PanelPage[] {
  const inRange = (page: number): boolean => Number.isInteger(page) && page >= 0 && page < pages.length;
  if (!inRange(toPage) || (drag.page !== null && !inRange(drag.page))) return pages;
  const fromOtherPage = drag.page !== null && drag.page !== toPage;
  const target = pageOptionalBlocks(pages[toPage]);
  const index = Math.min(Math.max(toIndex, 0), target.length);
  const from = target.indexOf(drag.id);
  if (!fromOtherPage && from !== -1 && (index === from || index === from + 1)) return pages;
  const blocks = target.filter((block) => block !== drag.id);
  blocks.splice(from !== -1 && from < index ? index - 1 : index, 0, drag.id);
  const next = [...pages];
  if (fromOtherPage && drag.page !== null) {
    const source = pages[drag.page];
    next[drag.page] = { ...source, blocks: pageOptionalBlocks(source).filter((block) => block !== drag.id) };
  }
  next[toPage] = { ...pages[toPage], blocks };
  return next;
}
