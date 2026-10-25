import { MarkerType, type Edge, type Node } from '@xyflow/react';
import type { WorkflowDef, WorkflowEdge, WorkflowIssue, WorkflowNode } from '../../lib/workflow';

// `type` (not `interface`) so the data objects carry an implicit index
// signature and satisfy React Flow's `Record<string, unknown>` constraint.
export type FlowNodeData = {
  node: WorkflowNode;
  edges: WorkflowEdge[];
  hasError: boolean;
};

export type EdgeStyle = 'forward' | 'loop' | 'exhausted';

export type FlowEdgeData = {
  edge: WorkflowEdge;
  hasError: boolean;
  style: EdgeStyle;
  /** X coordinate of the lane a loop edge takes beside the nodes. */
  laneX: number;
};

export type FlowNode = Node<FlowNodeData>;
export type FlowEdge = Edge<FlowEdgeData>;

export interface FlowSelection {
  nodeId: string | null;
  edgeId: string | null;
}

export function edgeId(edge: WorkflowEdge): string {
  return `${edge.from}-${edge.port}-${edge.to}`;
}

export const EDGE_COLORS: Record<EdgeStyle, string> = {
  forward: 'var(--muted)',
  loop: 'var(--accent)',
  exhausted: 'var(--err)'
};

export const EDGE_DASH: Record<EdgeStyle, string | undefined> = {
  forward: undefined,
  loop: undefined,
  exhausted: '2 4'
};

/** A paused edge is dashed whatever its style. */
export const PAUSE_DASH = '6 4';

export function edgeDash(edge: WorkflowEdge, style: EdgeStyle): string | undefined {
  if (edge.pause) return PAUSE_DASH;
  return EDGE_DASH[style];
}

export function toFlowNodes(
  def: WorkflowDef,
  issues: WorkflowIssue[],
  selection: FlowSelection,
  measured: Record<string, { width: number; height: number }>
): FlowNode[] {
  const errorNodeIds = new Set<string>();
  for (const issue of issues) {
    if (issue.nodeId) errorNodeIds.add(issue.nodeId);
  }
  return def.nodes.map((node) => ({
    id: node.id,
    type: node.kind,
    position: node.position,
    data: { node, edges: def.edges, hasError: errorNodeIds.has(node.id) },
    selected: node.id === selection.nodeId,
    measured: measured[node.id]
  }));
}

const LOOP_LANE_OFFSET = 40;
const LOOP_LANE = 28;

function nodeSize(
  nodeId: string,
  measured: Record<string, { width: number; height: number }>
): { width: number; height: number } {
  return measured[nodeId] ?? { width: 280, height: 80 };
}

function edgeStyle(edge: WorkflowEdge, source: WorkflowNode | undefined, target: WorkflowNode | undefined): EdgeStyle {
  if (edge.port === 'exhausted') return 'exhausted';
  if (source && target && target.position.y <= source.position.y) return 'loop';
  return 'forward';
}

function loopLaneX(
  nodes: WorkflowNode[],
  measured: Record<string, { width: number; height: number }>,
  source: WorkflowNode,
  target: WorkflowNode,
  lane: number
): number {
  const sourceHeight = nodeSize(source.id, measured).height;
  const top = target.position.y;
  const bottom = source.position.y + sourceHeight;
  let maxRight = Number.NEGATIVE_INFINITY;
  for (const node of nodes) {
    const { width, height } = nodeSize(node.id, measured);
    const nodeTop = node.position.y;
    if (nodeTop < bottom && nodeTop + height > top) {
      maxRight = Math.max(maxRight, node.position.x + width);
    }
  }
  if (!Number.isFinite(maxRight)) maxRight = 0;
  return maxRight + LOOP_LANE_OFFSET + lane * LOOP_LANE;
}

export function toFlowEdges(
  def: WorkflowDef,
  issues: WorkflowIssue[],
  selection: FlowSelection,
  measured: Record<string, { width: number; height: number }>
): FlowEdge[] {
  const errorEdges = new Set(issues.map((issue) => issue.edge).filter((edge) => edge != null));
  let loopLane = 0;
  return def.edges.map((edge, index) => {
    const sourceNode = def.nodes.find((node) => node.id === edge.from);
    const targetNode = def.nodes.find((node) => node.id === edge.to);
    const style = edgeStyle(edge, sourceNode, targetNode);
    const hasError = errorEdges.has(index);
    let laneX = 0;
    if (style === 'loop' && sourceNode && targetNode) {
      laneX = loopLaneX(def.nodes, measured, sourceNode, targetNode, loopLane);
      loopLane += 1;
    }
    const color = hasError ? 'var(--err)' : EDGE_COLORS[style];
    return {
      id: edgeId(edge),
      source: edge.from,
      sourceHandle: edge.port,
      target: edge.to,
      type: style === 'loop' ? 'loop' : 'default',
      data: { edge, hasError, style, laneX },
      selected: edgeId(edge) === selection.edgeId,
      markerEnd: { type: MarkerType.ArrowClosed, color }
    };
  });
}
