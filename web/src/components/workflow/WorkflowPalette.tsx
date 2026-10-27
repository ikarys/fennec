import { NODE_KIND_LABELS, type NodeKind } from '../../lib/workflow';

interface WorkflowPaletteProps {
  onAddNode: (kind: NodeKind) => void;
}

const KINDS: { kind: NodeKind; shape: string }[] = [
  { kind: 'start', shape: 'rounded-full bg-raised' },
  { kind: 'end', shape: 'rounded-full bg-ok-soft' },
  { kind: 'agent', shape: 'rounded-lg bg-panel border border-border' },
  { kind: 'shell', shape: 'rounded-lg bg-code' },
  { kind: 'criteria', shape: 'rounded-lg bg-code border border-ok/30' },
  { kind: 'human', shape: 'bg-accent-soft' },
  { kind: 'condition', shape: 'bg-raised' },
  { kind: 'parallel', shape: 'rounded-md bg-raised' },
  { kind: 'join', shape: 'rounded-md bg-raised' }
];

export function WorkflowPalette({ onAddNode }: WorkflowPaletteProps) {
  return (
    <aside className="flex w-40 shrink-0 flex-col gap-1 overflow-y-auto border-r border-border bg-panel p-2">
      <div className="px-1 pb-1 text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Nœuds</div>
      {KINDS.map(({ kind, shape }) => (
        <button
          key={kind}
          type="button"
          draggable
          onDragStart={(event) => {
            event.dataTransfer.setData('application/fennec-node', kind);
            event.dataTransfer.effectAllowed = 'move';
          }}
          onClick={() => onAddNode(kind)}
          className={`flex h-10 items-center gap-2 rounded-lg px-2 text-left text-[13px] text-text hover:bg-raised ${shape}`}
        >
          <span className="h-3 w-3 rounded-full bg-current opacity-30" />
          {NODE_KIND_LABELS[kind]}
        </button>
      ))}
    </aside>
  );
}
