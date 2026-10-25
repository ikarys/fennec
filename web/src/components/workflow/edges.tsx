import { Fragment } from 'react';
import { BaseEdge, EdgeLabelRenderer, getBezierPath, getSmoothStepPath, type EdgeProps } from '@xyflow/react';
import type { WorkflowEdge } from '../../lib/workflow';
import type { FlowEdge } from './flow';

// The port is already labelled on its node: the edge only shows what it adds.
function edgeLabel(edge: WorkflowEdge): string {
  const parts = [];
  if (edge.maxPasses != null) parts.push(`≤ ${edge.maxPasses} passages`);
  if (edge.pause) parts.push('pause');
  return parts.join(' · ');
}

function WorkflowEdgeBase({
  pathType,
  ...props
}: EdgeProps<FlowEdge> & { pathType: 'bezier' | 'smoothstep' }) {
  const { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, selected, markerEnd } = props;
  const edge = data?.edge;
  const label = edge ? edgeLabel(edge) : '';
  const isPaused = edge?.pause === true;
  const pathParams = { sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition };
  const [path, labelX, labelY] =
    pathType === 'smoothstep' ? getSmoothStepPath(pathParams) : getBezierPath(pathParams);

  return (
    <Fragment>
      <BaseEdge
        id={id}
        path={path}
        markerEnd={markerEnd}
        style={{
          strokeDasharray: isPaused ? '5,5' : undefined,
          stroke: data?.hasError ? 'var(--err)' : selected ? 'var(--accent)' : undefined
        }}
      />
      {label && <EdgeLabelRenderer>
        <div
          className="absolute rounded-md border border-border bg-panel px-1.5 py-0.5 text-[10px] text-muted"
          style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}
        >
          {label}
        </div>
      </EdgeLabelRenderer>}
    </Fragment>
  );
}

export const workflowEdgeTypes = {
  default: (props: EdgeProps<FlowEdge>) => <WorkflowEdgeBase pathType="bezier" {...props} />,
  smoothstep: (props: EdgeProps<FlowEdge>) => <WorkflowEdgeBase pathType="smoothstep" {...props} />
};
