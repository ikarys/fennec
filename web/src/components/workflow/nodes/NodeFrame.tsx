import { Fragment, type ReactNode } from 'react';
import { Handle, Position } from '@xyflow/react';
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
              className="absolute top-full text-[0.6875rem] whitespace-nowrap text-muted"
              style={{ left: `${percent}%`, transform: 'translateX(-50%)', marginTop: '4px' }}
            >
              {port}
            </span>
          </Fragment>
        );
      })}
    </div>
  );
}
