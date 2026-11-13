import { useCallback, useEffect, useMemo, useState, type DragEvent } from 'react';
import {
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange
} from '@xyflow/react';
import { useT } from '../../i18n';
import type { Agent } from '../../lib/api';
import { listAgents, saveWorkflow, validateWorkflow, WorkflowSaveError } from '../../lib/api';
import type { NodeKind, Origin, WorkflowDef, WorkflowEdge, WorkflowIssue, WorkflowNode } from '../../lib/workflow';
import { newNode, renameNodeId } from '../../lib/workflow';
import { edgeId, toFlowEdges, toFlowNodes, type FlowEdge, type FlowNode, type FlowSelection } from './flow';

interface UseWorkflowEditorArgs {
  initialDef: WorkflowDef;
  initialOrigin: Origin;
  projectId: string | null;
}

export function useWorkflowEditor({ initialDef, initialOrigin, projectId }: UseWorkflowEditorArgs) {
  const t = useT();
  const [def, setDef] = useState<WorkflowDef>(initialDef);
  const [origin, setOrigin] = useState<Origin>(initialOrigin);
  const [scope, setScope] = useState<'project' | 'global'>(initialOrigin === 'default' ? 'project' : initialOrigin);
  const [dirty, setDirty] = useState(false);
  const [issues, setIssues] = useState<WorkflowIssue[]>([]);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [selection, setSelection] = useState<FlowSelection>({ nodeId: null, edgeId: null });
  const [measured, setMeasured] = useState<Record<string, { width: number; height: number }>>({});
  const [agents, setAgents] = useState<Agent[]>([]);
  const { screenToFlowPosition, fitView } = useReactFlow();

  // Load agents for the agent select.
  useEffect(() => {
    if (!projectId) return;
    listAgents(projectId)
      .then(setAgents)
      .catch(() => setAgents([]));
  }, [projectId]);

  // Debounced validation as you type.
  useEffect(() => {
    if (!projectId) return;
    const timer = setTimeout(() => {
      validateWorkflow(projectId, def)
        .then(setIssues)
        .catch(() => setIssues([]));
    }, 400);
    return () => clearTimeout(timer);
  }, [def, projectId]);

  // beforeunload guard for unsaved changes.
  useEffect(() => {
    if (!dirty) return;
    const handler = (event: BeforeUnloadEvent) => {
      event.preventDefault();
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [dirty]);

  const markDirty = useCallback(() => {
    setDirty(true);
    setSaved(false);
    setSaveError(null);
  }, []);

  const updateDef = useCallback(
    (updater: (previous: WorkflowDef) => WorkflowDef) => {
      setDef((previous) => updater(previous));
      markDirty();
    },
    [markDirty]
  );

  // React Flow is controlled: it hides a node until its measured size comes
  // back, and selects only through these changes. Neither touches the def.
  const onNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      for (const change of changes) {
        if (change.type === 'dimensions' && change.dimensions) {
          const size = change.dimensions;
          setMeasured((previous) => ({ ...previous, [change.id]: size }));
        } else if (change.type === 'select') {
          setSelection((previous) =>
            change.selected
              ? { nodeId: change.id, edgeId: null }
              : previous.nodeId === change.id
                ? { ...previous, nodeId: null }
                : previous
          );
        }
      }
      const edits = changes.filter((change) => (change.type === 'position' && change.position) || change.type === 'remove');
      if (edits.length === 0) return;
      updateDef((previous) => {
        let nodes = previous.nodes;
        let edges = previous.edges;
        for (const change of edits) {
          if (change.type === 'position' && change.position) {
            const position = { x: Math.round(change.position.x), y: Math.round(change.position.y) };
            nodes = nodes.map((node) => (node.id === change.id ? { ...node, position } : node));
          } else if (change.type === 'remove') {
            nodes = nodes.filter((node) => node.id !== change.id);
            edges = edges.filter((edge) => edge.from !== change.id && edge.to !== change.id);
          }
        }
        return { ...previous, nodes, edges };
      });
    },
    [updateDef]
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange<FlowEdge>[]) => {
      for (const change of changes) {
        if (change.type === 'select') {
          setSelection((previous) =>
            change.selected
              ? { nodeId: null, edgeId: change.id }
              : previous.edgeId === change.id
                ? { ...previous, edgeId: null }
                : previous
          );
        }
      }
      const removed = new Set(changes.flatMap((change) => (change.type === 'remove' ? [change.id] : [])));
      if (removed.size === 0) return;
      updateDef((previous) => ({ ...previous, edges: previous.edges.filter((edge) => !removed.has(edgeId(edge))) }));
    },
    [updateDef]
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      updateDef((previous) => {
        const { source, sourceHandle, target } = connection;
        if (!source || !sourceHandle || !target) return previous;
        const port = sourceHandle;
        const isBranch = previous.nodes.find((node) => node.id === source)?.kind === 'parallel';
        const edges = isBranch
          ? previous.edges
          : previous.edges.filter((edge) => !(edge.from === source && edge.port === port));
        const newEdge: WorkflowEdge = { from: source, port, to: target };
        return { ...previous, edges: [...edges, newEdge] };
      });
      // A loop edge is routed in a right lane; refit so it stays in view.
      const sourceNode = def.nodes.find((node) => node.id === connection.source);
      const targetNode = def.nodes.find((node) => node.id === connection.target);
      if (sourceNode && targetNode && targetNode.position.y <= sourceNode.position.y) {
        requestAnimationFrame(() => fitView({ duration: 200 }));
      }
    },
    [updateDef, def, fitView]
  );

  const addNode = useCallback(
    (kind: NodeKind, position?: { x: number; y: number }) => {
      updateDef((previous) => {
        const node = newNode(kind, previous, position ?? { x: 100, y: 100 });
        return { ...previous, nodes: [...previous.nodes, node] };
      });
    },
    [updateDef]
  );

  const onDrop = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      const kind = event.dataTransfer.getData('application/fennec-node') as NodeKind;
      if (!kind) return;
      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      addNode(kind, position);
    },
    [screenToFlowPosition, addNode]
  );

  const onDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  const save = useCallback(async () => {
    if (!projectId || saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      const result = await saveWorkflow(projectId, def.id, scope, def);
      setDef(result.workflow);
      setOrigin(result.origin);
      setDirty(false);
      setSaved(true);
    } catch (error) {
      if (error instanceof WorkflowSaveError) {
        setIssues(error.issues);
        setSaveError(error.message);
      } else {
        setSaveError(error instanceof Error ? error.message : t('workflowSaveError'));
      }
    } finally {
      setSaving(false);
    }
  }, [projectId, def, scope, saving, t]);

  const updateNode = useCallback(
    (node: WorkflowNode) => {
      updateDef((previous) => ({
        ...previous,
        nodes: previous.nodes.map((n) => (n.id === node.id ? node : n))
      }));
    },
    [updateDef]
  );

  const renameNode = useCallback(
    (oldId: string, newId: string) => {
      updateDef((previous) => renameNodeId(previous, oldId, newId));
      setSelection((previous) => ({ ...previous, nodeId: newId }));
    },
    [updateDef]
  );

  const deleteNode = useCallback(
    (nodeId: string) => {
      updateDef((previous) => ({
        ...previous,
        nodes: previous.nodes.filter((n) => n.id !== nodeId),
        edges: previous.edges.filter((edge) => edge.from !== nodeId && edge.to !== nodeId)
      }));
      setSelection((previous) => ({ ...previous, nodeId: null }));
    },
    [updateDef]
  );

  const updateEdge = useCallback(
    (edge: WorkflowEdge) => {
      updateDef((previous) => ({
        ...previous,
        edges: previous.edges.map((e) =>
          e.from === edge.from && e.port === edge.port && e.to === edge.to ? edge : e
        )
      }));
    },
    [updateDef]
  );

  const deleteEdge = useCallback(
    (edge: WorkflowEdge) => {
      updateDef((previous) => ({
        ...previous,
        edges: previous.edges.filter(
          (e) => !(e.from === edge.from && e.port === edge.port && e.to === edge.to)
        )
      }));
      setSelection((previous) => ({ ...previous, edgeId: null }));
    },
    [updateDef]
  );

  const selectNode = useCallback((nodeId: string | null) => {
    setSelection({ nodeId, edgeId: null });
  }, []);

  const selectEdge = useCallback((edgeId: string | null) => {
    setSelection({ nodeId: null, edgeId });
  }, []);

  const flowNodes = useMemo(() => toFlowNodes(def, issues, selection, measured), [def, issues, selection, measured]);
  const flowEdges = useMemo(() => toFlowEdges(def, issues, selection, measured), [def, issues, selection, measured]);

  const selectedNode = def.nodes.find((node) => node.id === selection.nodeId) ?? null;
  const selectedEdge = def.edges.find((edge) => edgeId(edge) === selection.edgeId) ?? null;

  return {
    def,
    origin,
    scope,
    dirty,
    issues,
    saving,
    saved,
    saveError,
    selection,
    agents,
    flowNodes,
    flowEdges,
    selectedNode,
    selectedEdge,
    setScope,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNode,
    onDrop,
    onDragOver,
    save,
    updateNode,
    renameNode,
    deleteNode,
    updateEdge,
    deleteEdge,
    updateDef,
    selectNode,
    selectEdge
  };
}
