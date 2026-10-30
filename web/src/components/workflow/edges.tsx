import { Fragment, type CSSProperties } from 'react';
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from '@xyflow/react';
import type { WorkflowEdge } from '../../lib/workflow';
import { EDGE_COLORS, edgeDash, type FlowEdge } from './flow';

// The port is already labelled on its node: the edge only shows what it adds.
function edgeLabel(edge: WorkflowEdge): string {
  const parts = [];
  if (edge.maxPasses != null) parts.push(`≤ ${edge.maxPasses} passages`);
  if (edge.pause) parts.push('pause');
  return parts.join(' · ');
}

const LOOP_DROP = 24;
const CORNER = 12;

/**
 * An edge back up the graph: down out of the source, along the lane beside
 * the nodes, up, then down into the target. The lane may be on either side,
 * so each corner bends in the direction it is actually travelling.
 */
function loopPath(sx: number, sy: number, tx: number, ty: number, laneX: number): [string, number, number] {
  const bottom = sy + LOOP_DROP;
  const top = ty - LOOP_DROP;
  const r = CORNER;
  const toLane = laneX >= sx ? 1 : -1;
  const toTarget = tx >= laneX ? 1 : -1;
  const path = [
    `M ${sx} ${sy}`,
    `L ${sx} ${bottom - r}`,
    `Q ${sx} ${bottom} ${sx + toLane * r} ${bottom}`,
    `L ${laneX - toLane * r} ${bottom}`,
    `Q ${laneX} ${bottom} ${laneX} ${bottom - r}`,
    `L ${laneX} ${top + r}`,
    `Q ${laneX} ${top} ${laneX + toTarget * r} ${top}`,
    `L ${tx - toTarget * r} ${top}`,
    `Q ${tx} ${top} ${tx} ${top + r}`,
    `L ${tx} ${ty}`
  ].join(' ');
  return [path, laneX, (top + bottom) / 2];
}

function WorkflowEdgeBase({
  pathType,
  ...props
}: EdgeProps<FlowEdge> & { pathType: 'bezier' | 'loop' }) {
  const { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, selected, markerEnd } = props;
  const edge = data?.edge;
  const style = data?.style ?? 'forward';
  const label = edge ? edgeLabel(edge) : '';
  const pathParams = { sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition };
  const [path, labelX, labelY] =
    pathType === 'loop'
      ? loopPath(sourceX, sourceY, targetX, targetY, data?.laneX ?? 0)
      : getBezierPath(pathParams);
  const color = data?.hasError ? 'var(--err)' : EDGE_COLORS[style];
  const strokeWidth = selected ? 3 : style === 'loop' ? 2 : 1.5;
  const strokeDasharray = data?.hasError ? undefined : edge ? edgeDash(edge, style) : undefined;
  const labelStyle: CSSProperties = {
    transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`
  };
  if (pathType === 'loop') {
    labelStyle.color = color;
    labelStyle.borderColor = color;
  }

  return (
    <Fragment>
      <BaseEdge
        id={id}
        path={path}
        markerEnd={markerEnd}
        style={{ stroke: color, strokeWidth, strokeDasharray }}
      />
      {label && <EdgeLabelRenderer>
        <div
          className="absolute rounded-md border border-border bg-panel px-1.5 py-0.5 text-[0.6875rem] text-muted"
          style={labelStyle}
        >
          {label}
        </div>
      </EdgeLabelRenderer>}
    </Fragment>
  );
}

export const workflowEdgeTypes = {
  default: (props: EdgeProps<FlowEdge>) => <WorkflowEdgeBase pathType="bezier" {...props} />,
  loop: (props: EdgeProps<FlowEdge>) => <WorkflowEdgeBase pathType="loop" {...props} />
};
