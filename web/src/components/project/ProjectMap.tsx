import { useMemo, useRef, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  ControlButton,
  MarkerType,
  BaseEdge,
  type Node,
  type Edge,
  type EdgeProps,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Segmented } from '../ui/Segmented';
import { EyeIcon } from '../Icons';
import { ModuleDetails } from './ModuleDetails';
import type { ModuleMap } from '../../lib/api';

function RoutedEdge({
  data,
  markerEnd,
  style,
  label,
  labelStyle,
  labelBgStyle,
}: EdgeProps<Edge<{ path: string; labelX: number; labelY: number }>>) {
  return (
    <BaseEdge
      path={data!.path}
      markerEnd={markerEnd}
      style={style}
      label={label}
      labelX={data!.labelX}
      labelY={data!.labelY}
      labelStyle={labelStyle}
      labelBgStyle={labelBgStyle}
    />
  );
}

const edgeTypes = { routed: RoutedEdge };

export function ProjectMap({ map }: { map: ModuleMap }) {
  const groups = map.groups.map((group) => group.name);

  const [selectedGroup, setSelectedGroup] = useState<string | null>(null);
  const [focusedModuleId, setFocusedModuleId] = useState<string | null>(null);
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const details = map.modules.find((module) => module.id === detailsId) ?? null;
  const selected = groups.find((group) => group === selectedGroup) ?? groups[0] ?? '';

  const frame = useRef<HTMLDivElement>(null);

  const toggleFullscreen = () => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void frame.current?.requestFullscreen();
    }
  };

  const openModule = (id: string) => {
    const target = map.modules.find((module) => module.id === id);
    if (target) {
      setSelectedGroup(target.group);
      setFocusedModuleId(id);
      setDetailsId(id);
    }
  };

  const groupModuleIds = useMemo(
    () => new Set(map.modules.filter((module) => module.group === selected).map((module) => module.id)),
    [map.modules, selected],
  );
  const focus = focusedModuleId !== null && groupModuleIds.has(focusedModuleId) ? focusedModuleId : null;

  const nodes = useMemo<Node[]>(() => {
    const adjacent = new Set<string>();
    if (focus !== null) {
      for (const dependency of map.dependencies) {
        if ((dependency.from === focus || dependency.to === focus) &&
          groupModuleIds.has(dependency.from) && groupModuleIds.has(dependency.to)) {
          adjacent.add(dependency.from);
          adjacent.add(dependency.to);
        }
      }
    }
    return map.modules.filter((module) => module.group === selected).map((module) => ({
      id: module.id,
      position: { x: module.x, y: module.y },
      data: {
        label: (
          <div>
            <div className="flex items-center gap-1">
              <span className="min-w-0 grow truncate font-mono text-xs" title={module.id}>
                {module.id}
              </span>
              <button
                type="button"
                className="shrink-0 rounded p-0.5 text-muted hover:bg-accent-soft hover:text-accent-strong"
                title="À quoi correspond ce module"
                aria-label="À quoi correspond ce module"
                onClick={(event) => {
                  event.stopPropagation();
                  setDetailsId(module.id);
                }}
              >
                <EyeIcon className="h-3 w-3" />
              </button>
            </div>
            <div className="text-xs text-muted">
              {module.files} fichiers · {module.tests} testés{module.entry ? ' · entrée' : ''}
            </div>
          </div>
        ),
      },
      style: {
        width: 260,
        height: 46,
        boxSizing: 'border-box',
        background: module.entry ? 'var(--accent-soft)' : 'var(--raised)',
        color: 'var(--text)',
        border: `1px solid ${module.entry ? 'var(--accent)' : 'var(--border)'}`,
        borderRadius: 8,
        padding: '6px 8px',
        textAlign: 'left' as const,
        cursor: 'pointer',
        opacity: focus !== null && module.id !== focus && !adjacent.has(module.id) ? 0.3 : 1,
      },
    }));
  }, [map.modules, map.dependencies, selected, focus, groupModuleIds]);

  const edges = useMemo<Edge[]>(() =>
    map.dependencies
      .filter((dependency) => groupModuleIds.has(dependency.from) && groupModuleIds.has(dependency.to))
      .map((dependency) => {
        const base = {
          id: `${dependency.from}->${dependency.to}`,
          source: dependency.from,
          target: dependency.to,
          type: 'routed' as const,
          data: {
            path: dependency.path,
            labelX: dependency.label.x,
            labelY: dependency.label.y,
          },
          markerEnd: { type: MarkerType.ArrowClosed },
        };
        if (dependency.violation) {
          return {
            ...base,
            label: String(dependency.imports),
            style: { stroke: 'var(--err)' },
            labelStyle: { fill: 'var(--text)', fontSize: 10 },
            labelBgStyle: { fill: 'var(--panel)' },
          };
        }
        if (focus === null) return { ...base, style: { stroke: 'var(--muted)', opacity: 0.5 } };
        if (dependency.from === focus || dependency.to === focus) {
          return {
            ...base,
            label: String(dependency.imports),
            style: { stroke: 'var(--accent)' },
            labelStyle: { fill: 'var(--text)', fontSize: 10 },
            labelBgStyle: { fill: 'var(--panel)' },
          };
        }
        return { ...base, style: { stroke: 'var(--muted)', opacity: 0.08 } };
      }),
    [map.dependencies, groupModuleIds, focus],
  );

  if (map.modules.length === 0) {
    return <p className="text-xs text-muted">Aucun fichier source trouvé (le projet est-il un dépôt git ?).</p>;
  }

  return (
    <>
      {/* The tab's body is a flex column: the wrapper keeps the control at its own width. */}
      {groups.length > 1 && (
        <div className="flex">
          <Segmented
            label="Groupe de modules"
            segments={map.groups.map((group) => ({
              value: group.name,
              label: `${group.name === '' ? 'Autres' : group.name} · ${group.modules}`,
            }))}
            value={selected}
            onChange={(name) => {
              setSelectedGroup(name);
              setFocusedModuleId(null);
            }}
          />
        </div>
      )}
      {/* The handles are connection points for editing; this map is read-only and they hide the arrow heads. */}
      <div
        ref={frame}
        className="mt-3 flex h-[600px] overflow-hidden rounded-md border border-border bg-panel [&:fullscreen]:h-screen [&:fullscreen]:rounded-none [&_.react-flow__handle]:invisible"
      >
        <div className="h-full min-w-0 grow">
          <ReactFlow
            key={selected}
            nodes={nodes}
            edges={edges}
            edgeTypes={edgeTypes}
            fitView
            nodesDraggable={false}
            nodesConnectable={false}
            elementsSelectable={false}
            proOptions={{ hideAttribution: true }}
            minZoom={0.1}
            onNodeClick={(_event, node) => setFocusedModuleId(focus === node.id ? null : node.id)}
            onPaneClick={() => setFocusedModuleId(null)}
          >
            <Background color="var(--faint)" />
            <Controls showInteractive={false}>
              <ControlButton onClick={toggleFullscreen} title="Plein écran" aria-label="Plein écran">
                ⛶
              </ControlButton>
            </Controls>
          </ReactFlow>
        </div>
        {details && <ModuleDetails module={details} onOpen={openModule} onClose={() => setDetailsId(null)} />}
      </div>
      <p className="mt-2 text-xs text-muted">
        Cliquez un module pour ne voir que ses liens, sur l'œil pour savoir à quoi il correspond ; le nombre sur une
        flèche compte les fichiers qui importent.
      </p>
    </>
  );
}
