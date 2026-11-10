/**
 * The file tree in the project's Files tab: collapsible folders and files.
 */
import { useEffect, useState } from 'react';
import { ChevronRightIcon, FileIcon, FolderIcon } from '../../ui/Icons';
import { ancestorsOf, type FileTreeNode } from '../../../lib/fileTree';
import { useT } from '../../../i18n';

function rowClass(active: boolean) {
  return (
    'relative mb-px flex h-[30px] w-full items-center gap-2 rounded-md pl-2 pr-2.5 text-left text-sm transition-colors ' +
    (active
      ? 'bg-accent-soft font-semibold text-accent before:absolute before:-left-0.5 before:top-[7px] before:bottom-[7px] before:w-[2.5px] before:rounded-sm before:bg-accent'
      : 'text-muted hover:bg-raised hover:text-text')
  );
}

export function FileTree({
  nodes,
  selected,
  onSelect,
}: {
  nodes: FileTreeNode[];
  selected: string | null;
  onSelect: (path: string) => void;
}) {
  const [open, setOpen] = useState<Set<string>>(
    () => new Set(selected ? ancestorsOf(selected) : []),
  );
  const t = useT();

  useEffect(() => {
    if (!selected) return;

    setOpen((prev) => {
      const ancestors = ancestorsOf(selected);
      if (ancestors.every((ancestor) => prev.has(ancestor))) return prev;

      const next = new Set(prev);
      ancestors.forEach((ancestor) => next.add(ancestor));
      return next;
    });
  }, [selected]);

  const toggle = (path: string) => {
    setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const renderNode = (node: FileTreeNode, depth: number) => {
    const indent = { paddingLeft: `${depth * 12 + 8}px` };

    if (node.children === null) {
      const active = selected === node.path;
      return (
        <button
          key={node.path}
          type="button"
          style={indent}
          aria-current={active ? 'page' : undefined}
          title={node.tracked ? node.path : t('filesUntracked')}
          className={rowClass(active)}
          onClick={() => onSelect(node.path)}
        >
          <FileIcon className="h-3.5 w-3.5 shrink-0 text-muted" />
          <span className={`truncate ${node.tracked ? '' : 'italic'}`}>
            {node.name}
          </span>
        </button>
      );
    }

    const isOpen = open.has(node.path);
    return (
      <div key={node.path}>
        <button
          type="button"
          style={indent}
          aria-expanded={isOpen}
          title={node.path}
          className={rowClass(false)}
          onClick={() => toggle(node.path)}
        >
          <ChevronRightIcon
            className={`h-3.5 w-3.5 shrink-0 text-muted transition-transform ${
              isOpen ? 'rotate-90' : ''
            }`}
          />
          <FolderIcon className="h-3.5 w-3.5 shrink-0 text-accent" />
          <span className="truncate">{node.name}</span>
        </button>
        {isOpen ? node.children.map((child) => renderNode(child, depth + 1)) : null}
      </div>
    );
  };

  return (
    <div className="flex flex-col">
      {nodes.map((node) => renderNode(node, 0))}
    </div>
  );
}
