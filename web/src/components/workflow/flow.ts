import { MarkerType, type Edge, type Node } from '@xyflow/react';
import type { WorkflowDef, WorkflowEdge, WorkflowIssue, WorkflowNode } from '../../lib/workflow';

// `type` (not `interface`) so the data objects carry an implicit index
// signature and satisfy React Flow's `Record<string, unknown>` constraint.
export type FlowNodeData = {
  node: WorkflowNode;
  edges: WorkflowEdge[];
  hasError: boolean;
};

export type FlowEdgeData = {
  edge: WorkflowEdge;
  hasError: boolean;
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

export function toFlowEdges(def: WorkflowDef, issues: WorkflowIssue[], selection: FlowSelection): FlowEdge[] {
  const errorEdges = new Set(issues.map((issue) => issue.edge).filter((edge) => edge != null));
  return def.edges.map((edge, index) => {
    const sourceNode = def.nodes.find((node) => node.id === edge.from);
    const targetNode = def.nodes.find((node) => node.id === edge.to);
    const isBackward = sourceNode && targetNode && targetNode.position.y < sourceNode.position.y;
    return {
      id: edgeId(edge),
      source: edge.from,
      sourceHandle: edge.port,
      target: edge.to,
      type: isBackward ? 'smoothstep' : 'default',
      data: { edge, hasError: errorEdges.has(index) },
      selected: edgeId(edge) === selection.edgeId,
      markerEnd: { type: MarkerType.ArrowClosed }
    };
  });
}
