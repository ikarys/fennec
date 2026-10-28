/**
 * The context panel's building blocks.
 *
 * A block is one bounded section of the right-hand panel; a page is a named
 * list of optional block ids. The core blocks open the first page (the
 * default one) and cannot be removed from it: they carry the integration
 * decision (what will be integrated, the run's contract). The other pages
 * show only their own blocks, so they stay light.
 */
import type { PanelPage } from './api';

export type PanelBlockId = 'criteria' | 'modifiedFiles' | 'workspace' | 'tokens' | 'skills' | 'decisions' | 'tools' | 'vision';

export interface PanelBlock {
  id: PanelBlockId;
  title: string;
  core: boolean;
}

/** In display order: core blocks first. */
export const PANEL_BLOCKS: readonly PanelBlock[] = [
  { id: 'criteria', title: 'Critères', core: true },
  { id: 'modifiedFiles', title: 'Fichiers modifiés', core: true },
  { id: 'workspace', title: "Espace de l'agent", core: true },
  { id: 'skills', title: 'Skills', core: false },
  { id: 'decisions', title: 'Décisions', core: false },
  { id: 'tokens', title: 'Tokens', core: false },
  { id: 'tools', title: 'Outils de ce tour', core: false },
  { id: 'vision', title: 'Vision', core: false }
];

export const OPTIONAL_BLOCKS: readonly PanelBlock[] = PANEL_BLOCKS.filter((block) => !block.core);

export const DEFAULT_PANEL_PAGES: readonly PanelPage[] = [
  { name: 'Défaut', blocks: ['skills', 'decisions'] },
  { name: 'Stats', blocks: ['tokens', 'tools', 'vision'] }
];

/** The pages to show: the stored ones, or the defaults when none are stored. */
export function panelPages(stored: PanelPage[] | null | undefined): PanelPage[] {
  return stored ?? [...DEFAULT_PANEL_PAGES];
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
