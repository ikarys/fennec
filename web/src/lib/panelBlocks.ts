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

export type PanelBlockId = 'request' | 'criteria' | 'modifiedFiles' | 'workspace' | 'servers' | 'tokens' | 'skills' | 'decisions' | 'vision';

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
  { id: 'vision', titleKey: 'panelVision', core: false }
];

export const OPTIONAL_BLOCKS: readonly PanelBlock[] = PANEL_BLOCKS.filter((block) => !block.core);

/** The default pages, named in the UI's language; stored pages keep their names. */
export function defaultPanelPages(lang: Lang): PanelPage[] {
  return [
    { name: t(lang, 'panelPageDecision'), blocks: [] },
    { name: t(lang, 'panelPageContext'), blocks: ['skills', 'decisions', 'servers'] },
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
