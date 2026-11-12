import { Fragment, useEffect, useRef, type ReactNode } from 'react';
import { Handle, Position, useUpdateNodeInternals } from '@xyflow/react';
import type { WorkflowEdge, WorkflowNode } from '../../../lib/workflow';
import { hasInput, outputPorts } from '../../../lib/workflow';

interface NodeFrameProps {
  node: WorkflowNode;
  edges: WorkflowEdge[];
  selected: boolean;
  hasError: boolean;
  children: ReactNode;
}

export function NodeFrame({ node, edges, selected, hasError, children }: NodeFrameProps) {
  const ports = outputPorts(node, edges);
  const input = hasInput(node.kind);
  // A port that appears (exhausted, a new choice) must be measured, or edges keep the old positions.
  // Only on a change: measuring at mount would race the editor's first fitView.
  const updateNodeInternals = useUpdateNodeInternals();
  const portKey = ports.join('\n');
  const measuredPorts = useRef(portKey);
  useEffect(() => {
    if (measuredPorts.current === portKey) return;
    measuredPorts.current = portKey;
    updateNodeInternals(node.id);
  }, [portKey, node.id, updateNodeInternals]);
  const ring = hasError ? 'ring-2 ring-err' : selected ? 'ring-2 ring-accent' : '';

  return (
    <div className={`relative rounded-lg ${ring}`}>
      {input && (
        <Handle
          type="target"
          position={Position.Top}
          className="!h-2.5 !w-2.5 !rounded-full !border-2 !border-panel !bg-raised"
        />
      )}
      {children}
      {ports.map((port, index) => {
        const percent = ((index + 1) / (ports.length + 1)) * 100;
        // Stagger odd labels when there are many ports so they do not overlap.
        return (
          <Fragment key={port}>
            <Handle
              id={port}
              type="source"
              position={Position.Bottom}
              style={{ left: `${percent}%` }}
              className="!h-2.5 !w-2.5 !rounded-full !border-2 !border-panel !bg-raised"
            />
            <span
              className="absolute top-full text-xs whitespace-nowrap text-muted"
              style={{ left: `${percent}%`, transform: 'translateX(-50%)', marginTop: ports.length > 2 && index % 2 === 1 ? '18px' : '4px' }}
            >
              {port}
            </span>
          </Fragment>
        );
      })}
    </div>
  );
}
