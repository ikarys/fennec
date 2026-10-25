export type NodeKind = 'start' | 'agent' | 'shell' | 'human' | 'condition' | 'parallel' | 'join' | 'end';

/** How the editor names each kind, in the palette and the node panel. */
export const NODE_KIND_LABELS: Record<NodeKind, string> = {
  start: 'Démarrage',
  end: 'Fin',
  agent: 'Agent',
  shell: 'Shell',
  human: 'Humain',
  condition: 'Condition',
  parallel: 'Parallèle',
  join: 'Convergence'
};

export type Origin = 'project' | 'global' | 'default';

export interface WorkflowDef {
  id: string;
  name: string;
  description?: string;
  maxSteps?: number;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  [key: string]: unknown;
}

export interface WorkflowNode {
  id: string;
  kind: NodeKind;
  name?: string;
  position: { x: number; y: number };
  [key: string]: unknown;
}

export interface WorkflowEdge {
  from: string;
  port: string;
  to: string;
  maxPasses?: number;
  pause?: boolean;
  [key: string]: unknown;
}

export interface WorkflowIssue {
  nodeId?: string;
  /** The index of the edge in `edges`. */
  edge?: number;
  message: string;
}

export interface WorkflowSummary {
  id: string;
  name: string;
  description: string;
  origin: Origin;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}

export function outputPorts(node: WorkflowNode, edges: WorkflowEdge[]): string[] {
  let ports: string[];
  switch (node.kind) {
    case 'agent': {
      const outputs = asStringArray(node.outputs);
      ports = outputs.length > 0 ? outputs : ['done'];
      break;
    }
    case 'shell':
      ports = ['success', 'failure'];
      break;
    case 'human':
      ports = asStringArray(node.choices);
      break;
    case 'condition':
      ports = ['true', 'false'];
      break;
    case 'parallel':
      ports = ['branch'];
      break;
    case 'start':
    case 'join':
      ports = ['next'];
      break;
    case 'end':
      ports = [];
      break;
  }
  // First, so it sits on the left: its label then spills out of the node
  // instead of over its neighbour's, and its exit (usually to a side end)
  // does not cross the loop that leaves on the right.
  if (edges.some((edge) => edge.from === node.id && edge.maxPasses != null)) {
    ports = ['exhausted', ...ports];
  }
  return ports;
}

export function hasInput(kind: NodeKind): boolean {
  return kind !== 'start';
}

export function templateNames(def: WorkflowDef, nodeId: string): string[] {
  const names = ['task', 'filesChanged', 'pass', 'comment'];
  for (const node of def.nodes) {
    if (node.id === nodeId) continue;
    names.push(`nodes.${node.id}.output`, `nodes.${node.id}.port`);
    if (node.kind === 'shell') {
      names.push(`nodes.${node.id}.exitCode`);
    }
  }
  return names;
}

function uniqueId(kind: NodeKind, nodes: WorkflowNode[]): string {
  const taken = new Set(nodes.map((node) => node.id));
  if (!taken.has(kind)) return kind;
  let n = 2;
  while (taken.has(`${kind}-${n}`)) n++;
  return `${kind}-${n}`;
}

export function newNode(kind: NodeKind, def: WorkflowDef, position: { x: number; y: number }): WorkflowNode {
  const id = uniqueId(kind, def.nodes);
  const node: WorkflowNode = { id, kind, position };
  switch (kind) {
    case 'agent':
      node.agent = 'builder';
      node.prompt = '{{task}}';
      break;
    case 'shell':
      node.command = '';
      break;
    case 'human':
      node.question = '';
      node.choices = ['Go', 'Stop'];
      break;
    case 'condition':
      node.value = '';
      node.test = 'equals';
      node.against = '';
      break;
    case 'end':
      node.outcome = 'done';
      break;
    default:
      break;
  }
  return node;
}

export function blankWorkflow(id: string): WorkflowDef {
  return {
    id,
    name: id,
    nodes: [
      { id: 'start', kind: 'start', position: { x: 0, y: 0 } },
      { id: 'done', kind: 'end', outcome: 'done', position: { x: 0, y: 200 } }
    ],
    edges: []
  };
}

export function renameNodeId(def: WorkflowDef, oldId: string, newId: string): WorkflowDef {
  const ref = `nodes.${oldId}.`;
  const replacement = `nodes.${newId}.`;
  const renameInStrings = (node: WorkflowNode): WorkflowNode => {
    const result: WorkflowNode = { ...node };
    for (const key of Object.keys(result)) {
      const value = result[key];
      if (typeof value === 'string' && value.includes(ref)) {
        (result as Record<string, unknown>)[key] = value.split(ref).join(replacement);
      }
    }
    return result;
  };
  const nodes = def.nodes.map((node) => (node.id === oldId ? renameInStrings({ ...node, id: newId }) : renameInStrings(node)));
  const edges = def.edges.map((edge) => ({
    ...edge,
    from: edge.from === oldId ? newId : edge.from,
    to: edge.to === oldId ? newId : edge.to
  }));
  return { ...def, nodes, edges };
}
