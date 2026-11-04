import type { PanelPage } from '../../lib/api';
import { OPTIONAL_BLOCKS, PANEL_BLOCKS, pageOptionalBlocks, panelBlock, panelPages, type PanelBlockId } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section, type SectionSave } from '../ui/Section';
import { Badge } from '../ui/Badge';
import { ChevronDownIcon, ChevronUpIcon, CloseIcon, PlusIcon, TrashIcon } from '../Icons';
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
                <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent text-xs font-semibold text-on-accent">
                  {index + 1}
                </span>
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
              <ol className="flex flex-col gap-1 p-2">
                {blocks.length === 0 && <li className="px-2 py-3 text-xs text-faint">{t('panelPageEmpty')}</li>}
                {blocks.map((id, blockIndex) => (
                  <li
                    key={id}
                    className="group flex items-center gap-2 rounded-md px-2 py-1 text-sm text-text transition-colors hover:bg-accent-soft"
                  >
                    <span className="w-4 text-right text-xs tabular-nums text-faint">{blockIndex + 1}</span>
                    <span className="grow group-hover:text-accent-strong">{t(panelBlock(id).titleKey)}</span>
                    <div className="flex items-center opacity-60 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                      <Button
                        variant="icon"
                        size="sm"
                        iconOnly
                        onClick={() => updatePage(index, { ...page, blocks: moveBlock(blocks, id, -1) })}
                        disabled={blockIndex === 0}
                        aria-label={t('panelMoveUp')}
                        title={t('panelMoveUp')}
                      >
                        <ChevronUpIcon className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="icon"
                        size="sm"
                        iconOnly
                        onClick={() => updatePage(index, { ...page, blocks: moveBlock(blocks, id, 1) })}
                        disabled={blockIndex === blocks.length - 1}
                        aria-label={t('panelMoveDown')}
                        title={t('panelMoveDown')}
                      >
                        <ChevronDownIcon className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        iconOnly
                        onClick={() => updatePage(index, { ...page, blocks: toggleBlock(blocks, id) })}
                        aria-label={t('panelBlockHide')}
                        title={t('panelBlockHide')}
                        className="hover:text-err"
                      >
                        <CloseIcon className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </li>
                ))}
              </ol>
              {available.length > 0 && (
                <div className="mt-auto flex flex-wrap items-center gap-1.5 border-t border-border px-3 py-2">
                  <span className="text-xs text-faint">{t('panelBlocksAvailable')}</span>
                  {available.map((block) => (
                    <button
                      key={block.id}
                      type="button"
                      onClick={() => updatePage(index, { ...page, blocks: toggleBlock(blocks, block.id) })}
                      className="inline-flex items-center gap-1 rounded-full border border-dashed border-border px-2 py-0.5 text-xs text-muted transition-colors hover:border-accent hover:bg-accent-soft hover:text-accent-strong"
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
