import { NODE_KIND_LABELS, type NodeKind } from '../../lib/workflow';
import { SectionLabel } from '../ui/SectionLabel';

interface WorkflowPaletteProps {
  onAddNode: (kind: NodeKind) => void;
}

const KINDS: { kind: NodeKind; dot: string }[] = [
  { kind: 'start', dot: 'bg-muted' },
  { kind: 'end', dot: 'bg-ok' },
  { kind: 'agent', dot: 'bg-accent' },
  { kind: 'shell', dot: 'bg-text' },
  { kind: 'criteria', dot: 'bg-ok' },
  { kind: 'human', dot: 'bg-accent-strong' },
  { kind: 'condition', dot: 'bg-faint' },
  { kind: 'parallel', dot: 'bg-faint' },
  { kind: 'join', dot: 'bg-faint' }
];

export function WorkflowPalette({ onAddNode }: WorkflowPaletteProps) {
  return (
    <aside className="flex w-40 shrink-0 flex-col gap-1 overflow-y-auto border-r border-border bg-panel p-2">
      <SectionLabel className="px-1 pb-1">Nœuds</SectionLabel>
      {KINDS.map(({ kind, dot }) => (
        <button
          key={kind}
          type="button"
          draggable
          onDragStart={(event) => {
            event.dataTransfer.setData('application/fennec-node', kind);
            event.dataTransfer.effectAllowed = 'move';
          }}
          onClick={() => onAddNode(kind)}
          className="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-sm text-text hover:bg-raised"
        >
          <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${dot}`} />
          {NODE_KIND_LABELS[kind]}
        </button>
      ))}
    </aside>
  );
}
