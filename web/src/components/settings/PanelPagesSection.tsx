import type { PanelPage } from '../../lib/api';
import { OPTIONAL_BLOCKS, PANEL_BLOCKS, pageOptionalBlocks, panelBlock, panelPages, type PanelBlockId } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { Section } from '../ui/Section';
import { TextInput } from '../ui/Field';

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
}

export function PanelPagesSection({ value, onChange }: PanelPagesSectionProps) {
  const pages = panelPages(value);
  const alwaysShown = `La première page affiche toujours : ${PANEL_BLOCKS.filter((block) => block.core).map((block) => block.title).join(', ')}.`;

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
    <Section>
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Panneau de contexte</h2>
        <p className="m-0 text-xs text-muted">{alwaysShown}</p>
        <div className="flex flex-col gap-4">
          {pages.map((page, index) => {
            // Edits work on what the panel shows, so unknown stored ids drop out on the first change.
            const blocks = pageOptionalBlocks(page);
            return (
              <div key={index} className="flex flex-col gap-2">
                <div className="flex items-center gap-2">
                  <TextInput
                    aria-label="Nom de la page"
                    value={page.name}
                    onChange={(event) => updatePage(index, { ...page, name: event.target.value })}
                    className="grow"
                  />
                  <Button variant="ghost" size="sm" onClick={() => removePage(index)} disabled={index === 0}>
                    Supprimer
                  </Button>
                </div>
                <div className="flex flex-col gap-1.5">
                  {blocks.map((id, blockIndex) => (
                    <div key={id} className="flex items-center gap-2 text-xs text-text">
                      <label className="flex grow items-center gap-2">
                        <input
                          type="checkbox"
                          checked
                          onChange={() => updatePage(index, { ...page, blocks: toggleBlock(blocks, id) })}
                          className="h-4 w-4 accent-[var(--accent)]"
                        />
                        <span>{panelBlock(id).title}</span>
                      </label>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => updatePage(index, { ...page, blocks: moveBlock(blocks, id, -1) })}
                        disabled={blockIndex === 0}
                        aria-label="Monter"
                      >
                        ↑
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => updatePage(index, { ...page, blocks: moveBlock(blocks, id, 1) })}
                        disabled={blockIndex === blocks.length - 1}
                        aria-label="Descendre"
                      >
                        ↓
                      </Button>
                    </div>
                  ))}
                  {OPTIONAL_BLOCKS.filter((block) => !blocks.includes(block.id)).map((block) => (
                    <label key={block.id} className="flex items-center gap-2 text-xs text-text">
                      <input
                        type="checkbox"
                        checked={false}
                        onChange={() => updatePage(index, { ...page, blocks: toggleBlock(blocks, block.id) })}
                        className="h-4 w-4 accent-[var(--accent)]"
                      />
                      <span className="grow">{block.title}</span>
                    </label>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" size="sm" onClick={addPage} disabled={pages.length >= MAX_PAGES}>
            Ajouter une page
          </Button>
          <Button variant="ghost" size="sm" onClick={() => onChange(null)} disabled={value === null}>
            Réinitialiser
          </Button>
        </div>
      </div>
    </Section>
  );
}
