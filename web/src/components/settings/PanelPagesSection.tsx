import type { PanelPage } from '../../lib/api';
import { useState, type DragEvent, type KeyboardEvent } from 'react';
import { OPTIONAL_BLOCKS, PANEL_BLOCKS, dropBlock, pageOptionalBlocks, panelBlock, panelPages, type BlockDrag, type PanelBlockId } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section, type SectionSave } from '../ui/Section';
import { Badge } from '../ui/Badge';
import { CloseIcon, GripIcon, PlusIcon, TrashIcon } from '../Icons';
import { useLang, useT } from '../../i18n';

const MAX_PAGES = 12;

function moveBlock(blocks: PanelBlockId[], id: PanelBlockId, direction: -1 | 1): string[] {
  const index = blocks.indexOf(id);
  if (index === -1) return blocks;
  const target = index + direction;
  if (target < 0 || target >= blocks.length) return blocks;
  const next = [...blocks];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

function toggleBlock(blocks: PanelBlockId[], id: PanelBlockId): PanelBlockId[] {
  if (blocks.includes(id)) {
    return blocks.filter((block) => block !== id);
  }
  return [...blocks, id];
}

function uniquePageName(pages: PanelPage[]): string {
  const used = new Set(pages.map((page) => page.name));
  let number = 1;
  let candidate = `Page ${number}`;
  while (used.has(candidate)) {
    number += 1;
    candidate = `Page ${number}`;
  }
  return candidate;
}

interface PanelPagesSectionProps {
  value: PanelPage[] | null;
  onChange: (value: PanelPage[] | null) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function PanelPagesSection({ value, onChange, save }: PanelPagesSectionProps) {
  const t = useT();
  const lang = useLang();
  const pages = panelPages(lang, value);
  const alwaysShown = t('panelFirstPageAlways', {
    blocks: PANEL_BLOCKS.filter((block) => block.core).map((block) => t(block.titleKey)).join(', ')
  });

  const updatePage = (index: number, next: PanelPage): void => {
    onChange(pages.map((page, i) => (i === index ? next : page)));
  };

  const removePage = (index: number): void => {
    // The first page carries the core blocks: it can be renamed, not removed.
    if (index === 0) return;
    onChange(pages.filter((_, i) => i !== index));
  };

  const addPage = (): void => {
    if (pages.length >= MAX_PAGES) return;
    onChange([...pages, { name: uniquePageName(pages), blocks: [] }]);
  };

  const [drag, setDrag] = useState<BlockDrag | null>(null);
  const [over, setOver] = useState<{ page: number; index: number } | null>(null);

  const startDrag = (event: DragEvent, next: BlockDrag): void => {
    event.dataTransfer.effectAllowed = 'move';
    // Firefox starts a drag only when some data is set.
    event.dataTransfer.setData('text/plain', next.id);
    setDrag(next);
  };

  const endDrag = (): void => {
    setDrag(null);
    setOver(null);
  };

  /** Over a row: before it in its upper half, after it in its lower half. */
  const overRow = (event: DragEvent<HTMLElement>, page: number, index: number): void => {
    if (!drag) return;
    event.preventDefault();
    event.stopPropagation();
    const box = event.currentTarget.getBoundingClientRect();
    setOver({ page, index: event.clientY > box.top + box.height / 2 ? index + 1 : index });
  };

  const overPage = (event: DragEvent, page: number, end: number): void => {
    if (!drag) return;
    event.preventDefault();
    if (over?.page !== page) setOver({ page, index: end });
  };

  const drop = (event: DragEvent): void => {
    event.preventDefault();
    if (drag && over) onChange(dropBlock(pages, drag, over.page, over.index));
    endDrag();
  };

  /** A row dragged back onto the chips leaves its page. */
  const dropOnChips = (event: DragEvent, page: number): void => {
    event.preventDefault();
    if (drag?.page === page) {
      const blocks = pageOptionalBlocks(pages[page]);
      updatePage(page, { ...pages[page], blocks: toggleBlock(blocks, drag.id) });
    }
    endDrag();
  };

  /** The keyboard's drag: Alt+↑/↓ moves the focused block. */
  const moveWithKeys = (event: KeyboardEvent, page: number, blocks: PanelBlockId[], id: PanelBlockId): void => {
    if (!event.altKey || (event.key !== 'ArrowUp' && event.key !== 'ArrowDown')) return;
    event.preventDefault();
    updatePage(page, { ...pages[page], blocks: moveBlock(blocks, id, event.key === 'ArrowUp' ? -1 : 1) });
  };

  return (
    <Section title={t('contextPanelTitle')} description={alwaysShown} save={save}>
      <div className="grid gap-3 lg:grid-cols-2">
        {pages.map((page, index) => {
          // Edits work on what the panel shows, so unknown stored ids drop out on the first change.
          const blocks = pageOptionalBlocks(page);
          const available = OPTIONAL_BLOCKS.filter((block) => !blocks.includes(block.id));
          return (
            <Card key={index} className="flex flex-col overflow-hidden transition-colors focus-within:border-accent/60">
              <div className="flex items-center gap-2 border-b border-border bg-accent-soft/40 px-3 py-2">
                <Badge appearance="solid">{index + 1}</Badge>
                <input
                  aria-label={t('panelPageNameLabel')}
                  value={page.name}
                  onChange={(event) => updatePage(index, { ...page, name: event.target.value })}
                  className="min-w-0 grow rounded-md bg-transparent px-1.5 py-1 text-sm font-medium text-text outline-none hover:bg-raised focus:bg-raised focus:ring-1 focus:ring-accent/60"
                />
                <Badge tone="accent">{blocks.length}</Badge>
                {index > 0 && (
                  <Button
                    variant="ghost"
                    size="sm"
                    iconOnly
                    aria-label={t('panelPageRemoveTitle')}
                    title={t('panelPageRemoveTitle')}
                    onClick={() => removePage(index)}
                    className="hover:text-err"
                  >
                    <TrashIcon className="h-4 w-4" />
                  </Button>
                )}
              </div>
              <ol
                className="flex grow flex-col p-2"
                onDragOver={(event) => overPage(event, index, blocks.length)}
                onDrop={drop}
              >
                {blocks.length === 0 && (
                  <li className={`rounded-md border border-dashed px-2 py-3 text-center text-xs ${over?.page === index ? 'border-accent bg-accent-soft text-accent-strong' : 'border-transparent text-faint'}`}>
                    {t('panelPageEmpty')}
                  </li>
                )}
                {blocks.map((id, blockIndex) => (
                  <li
                    key={id}
                    draggable
                    tabIndex={0}
                    title={t('panelBlockDrag')}
                    onDragStart={(event) => startDrag(event, { id, page: index })}
                    onDragEnd={endDrag}
                    onDragOver={(event) => overRow(event, index, blockIndex)}
                    onKeyDown={(event) => moveWithKeys(event, index, blocks, id)}
                    className={`group relative flex cursor-grab items-center gap-2 rounded-md px-2 py-1.5 text-sm text-text transition-colors outline-none hover:bg-accent-soft focus-visible:ring-1 focus-visible:ring-accent active:cursor-grabbing ${drag?.page === index && drag.id === id ? 'opacity-40' : ''}`}
                  >
                    {over?.page === index && over.index === blockIndex && <DropLine edge="top" />}
                    {over?.page === index && over.index === blocks.length && blockIndex === blocks.length - 1 && <DropLine edge="bottom" />}
                    <GripIcon className="h-4 w-4 shrink-0 text-faint group-hover:text-accent" />
                    <span className="grow group-hover:text-accent-strong">{t(panelBlock(id).titleKey)}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      iconOnly
                      onClick={() => updatePage(index, { ...page, blocks: toggleBlock(blocks, id) })}
                      aria-label={t('panelBlockHide')}
                      title={t('panelBlockHide')}
                      className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover:text-err"
                    >
                      <CloseIcon className="h-3.5 w-3.5" />
                    </Button>
                  </li>
                ))}
              </ol>
              {(available.length > 0 || drag?.page === index) && (
                <div
                  onDragOver={(event) => drag?.page === index && event.preventDefault()}
                  onDrop={(event) => dropOnChips(event, index)}
                  className={`flex flex-wrap items-center gap-1.5 border-t px-3 py-2 transition-colors ${drag?.page === index ? 'border-err/40 bg-err-soft' : 'border-border'}`}
                >
                  <span className={`text-xs ${drag?.page === index ? 'text-err' : 'text-faint'}`}>
                    {t(drag?.page === index ? 'panelDropToRemove' : 'panelBlocksAvailable')}
                  </span>
                  {available.map((block) => (
                    <button
                      key={block.id}
                      type="button"
                      draggable
                      onDragStart={(event) => startDrag(event, { id: block.id, page: null })}
                      onDragEnd={endDrag}
                      onClick={() => updatePage(index, { ...page, blocks: toggleBlock(blocks, block.id) })}
                      className="inline-flex cursor-grab items-center gap-1 rounded-full border border-dashed border-border px-2 py-0.5 text-xs text-muted active:cursor-grabbing transition-colors hover:border-accent hover:bg-accent-soft hover:text-accent-strong"
                    >
                      <PlusIcon className="h-3 w-3" />
                      {t(block.titleKey)}
                    </button>
                  ))}
                </div>
              )}
            </Card>
          );
        })}
        {pages.length < MAX_PAGES && (
          <button
            type="button"
            onClick={addPage}
            className="flex min-h-24 items-center justify-center gap-2 rounded-xl border border-dashed border-border text-sm text-muted transition-colors hover:border-accent hover:bg-accent-soft hover:text-accent-strong"
          >
            <PlusIcon className="h-4 w-4" />
            {t('panelAddPage')}
          </button>
        )}
      </div>
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={() => onChange(null)} disabled={value === null}>
          {t('reset')}
        </Button>
      </div>
    </Section>
  );
}

/** Where a dragged block will land: an accent line on the edge of a row. */
function DropLine({ edge }: { edge: 'top' | 'bottom' }) {
  return <span className={`pointer-events-none absolute inset-x-1 h-0.5 rounded-full bg-accent ${edge === 'top' ? '-top-px' : '-bottom-px'}`} />;
}
