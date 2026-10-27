import { Fragment } from 'react';
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  type EdgeProps,
  type EdgeTypes,
  type NodeProps,
  type NodeTypes
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { WorkflowDef, WorkflowNode } from '../../lib/workflow';
import { EditorHeader } from './EditorHeader';
import { EdgeLegend } from './EdgeLegend';
import { EdgePanel } from './EdgePanel';
import { workflowEdgeTypes } from './edges';
import { NodePanel } from './NodePanel';
import { NodeFrame } from './nodes/NodeFrame';
import {
  AgentShape,
  ConditionShape,
  CriteriaShape,
  EndShape,
  HumanShape,
  JoinShape,
  ParallelShape,
  ShellShape,
  StartShape
} from './nodes/shapes';
import { WorkflowPalette } from './WorkflowPalette';
import { useWorkflowEditor } from './useWorkflowEditor';
import { edgeId, type FlowEdge, type FlowNode } from './flow';

function makeNodeComponent(shape: (props: { node: WorkflowNode }) => React.ReactNode) {
  return function NodeComponent({ data, selected }: NodeProps<FlowNode>) {
    const { node, edges, hasError } = data;
    return (
      <NodeFrame node={node} edges={edges} selected={selected} hasError={hasError}>
        {shape({ node })}
      </NodeFrame>
    );
  };
}

// Module-level so React Flow does not re-create the map (and warn) on render.
const nodeTypes = {
  start: makeNodeComponent(StartShape),
  end: makeNodeComponent(EndShape),
  agent: makeNodeComponent(AgentShape),
  shell: makeNodeComponent(ShellShape),
  criteria: makeNodeComponent(CriteriaShape),
  human: makeNodeComponent(HumanShape),
  condition: makeNodeComponent(ConditionShape),
  parallel: makeNodeComponent(ParallelShape),
  join: makeNodeComponent(JoinShape)
} satisfies NodeTypes;

const edgeTypes = {
  default: (props: EdgeProps<FlowEdge>) => <Fragment>{workflowEdgeTypes.default(props)}</Fragment>,
  loop: (props: EdgeProps<FlowEdge>) => <Fragment>{workflowEdgeTypes.loop(props)}</Fragment>
} satisfies EdgeTypes;

interface WorkflowEditorProps {
  initialDef: WorkflowDef;
  initialOrigin: 'project' | 'global' | 'default';
  projectId: string | null;
}

function WorkflowEditorInner({ initialDef, initialOrigin, projectId }: WorkflowEditorProps) {
  const editor = useWorkflowEditor({ initialDef, initialOrigin, projectId });

  return (
    <div className="flex h-full min-w-0 grow flex-col">
      <EditorHeader
        def={editor.def}
        origin={editor.origin}
        scope={editor.scope}
        dirty={editor.dirty}
        issueCount={editor.issues.length}
        saving={editor.saving}
        onNameChange={(name) => editor.updateDef((previous) => ({ ...previous, name }))}
        onDescriptionChange={(description) => editor.updateDef((previous) => ({ ...previous, description }))}
        onScopeChange={editor.setScope}
        onSave={editor.save}
      />

      <div className="flex min-h-0 grow">
        <WorkflowPalette onAddNode={(kind) => editor.addNode(kind)} />
        <div className="relative min-w-0 grow">
          <ReactFlow<FlowNode, FlowEdge>
            nodes={editor.flowNodes}
            edges={editor.flowEdges}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            onNodesChange={editor.onNodesChange}
            onEdgesChange={editor.onEdgesChange}
            onConnect={editor.onConnect}
            fitView
            deleteKeyCode={['Backspace', 'Delete']}
            onDrop={editor.onDrop}
            onDragOver={editor.onDragOver}
          >
            <Background color="var(--faint)" />
            <EdgeLegend />
            <Controls />
            <MiniMap bgColor="var(--panel)" nodeColor="var(--raised)" maskColor="var(--scrim)" pannable zoomable />
          </ReactFlow>
        </div>
        <aside className="w-72 shrink-0 overflow-y-auto border-l border-border bg-panel">
          {editor.selectedNode ? (
            <NodePanel
              def={editor.def}
              node={editor.selectedNode}
              agents={editor.agents}
              issues={editor.issues}
              onChange={editor.updateNode}
              onRename={editor.renameNode}
              onDelete={() => editor.deleteNode(editor.selectedNode!.id)}
            />
          ) : editor.selectedEdge ? (
            <EdgePanel
              edge={editor.selectedEdge}
              issues={editor.issues.filter((issue) => issue.edge === editor.def.edges.indexOf(editor.selectedEdge!))}
              onChange={editor.updateEdge}
              onDelete={() => editor.deleteEdge(editor.selectedEdge!)}
            />
          ) : (
            <div className="flex flex-col gap-3 p-3">
              <div>
                <label className="mb-1 block text-[0.8125rem] text-muted">Nom</label>
                <input
                  type="text"
                  value={editor.def.name}
                  onChange={(event) => editor.updateDef((previous) => ({ ...previous, name: event.target.value }))}
                  className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
                />
              </div>
              <div>
                <label className="mb-1 block text-[0.8125rem] text-muted">Description</label>
                <textarea
                  value={editor.def.description ?? ''}
                  onChange={(event) =>
                    editor.updateDef((previous) => ({ ...previous, description: event.target.value }))
                  }
                  rows={2}
                  className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
                />
              </div>
              <div>
                <label className="mb-1 block text-[0.8125rem] text-muted">Passages max</label>
                <input
                  type="number"
                  value={editor.def.maxSteps ?? 50}
                  onChange={(event) =>
                    editor.updateDef((previous) => ({ ...previous, maxSteps: Number(event.target.value) }))
                  }
                  className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
                />
              </div>
              {editor.issues.length > 0 && (
                <div className="flex flex-col gap-1">
                  <div className="text-[0.8125rem] font-medium text-err">Problèmes</div>
                  {editor.issues.map((issue, index) => (
                    <button
                      key={index}
                      type="button"
                      onClick={() => {
                        if (issue.nodeId) {
                          editor.selectNode(issue.nodeId);
                        } else if (issue.edge != null && editor.def.edges[issue.edge]) {
                          editor.selectEdge(edgeId(editor.def.edges[issue.edge]));
                        }
                      }}
                      className="rounded-md bg-del-bg px-2 py-1 text-left text-xs text-err hover:bg-del-bg/80"
                    >
                      {issue.message}
                    </button>
                  ))}
                </div>
              )}
              {editor.saveError && <div className="text-xs text-err">{editor.saveError}</div>}
              {editor.saved && <div className="text-xs text-ok">Enregistré</div>}
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}

export function WorkflowEditor(props: WorkflowEditorProps) {
  return (
    <ReactFlowProvider>
      <WorkflowEditorInner {...props} />
    </ReactFlowProvider>
  );
}
