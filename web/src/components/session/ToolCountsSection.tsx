import type { ThreadView } from '../../lib/fold';
import { panelBlock } from '../../lib/panelBlocks';
import { Panel } from '../ui/Panel';

interface ToolCountsSectionProps {
  toolCounts: ThreadView['toolCounts'];
}

export function ToolCountsSection({ toolCounts }: ToolCountsSectionProps) {
  const total = toolCounts.reduce((sum, tool) => sum + tool.count, 0);

  return (
    <Panel id="tools" title={panelBlock('tools').title} summary={total > 0 ? total : undefined}>
      {toolCounts.length === 0 && <div className="text-xs text-muted">Aucun outil appelé.</div>}
      {toolCounts.map((tool) => (
        <div key={tool.name} className="flex justify-between text-xs">
          <span className="font-mono">{tool.name}</span>
          <span className="text-muted tabular-nums">{tool.count}</span>
        </div>
      ))}
    </Panel>
  );
}
