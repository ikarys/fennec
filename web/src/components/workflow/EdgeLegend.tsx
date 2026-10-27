import { Panel } from '@xyflow/react';
import { EDGE_COLORS, EDGE_DASH, PAUSE_DASH } from './flow';

const items = [
  { label: 'Suite', color: EDGE_COLORS.forward, dash: EDGE_DASH.forward },
  { label: 'Boucle (retour)', color: EDGE_COLORS.loop, dash: EDGE_DASH.loop },
  { label: 'Épuisé (exhausted)', color: EDGE_COLORS.exhausted, dash: EDGE_DASH.exhausted },
  { label: 'Pause', color: EDGE_COLORS.forward, dash: PAUSE_DASH }
];

export function EdgeLegend() {
  return (
    <Panel position="top-left">
      <div className="flex flex-col gap-1 rounded-md border border-border bg-panel/90 px-2 py-1.5 text-[0.6875rem] text-muted">
        {items.map((item) => (
          <div key={item.label} className="flex items-center gap-2">
            <svg width="28" height="8" aria-hidden="true">
              <line
                x1="0"
                y1="4"
                x2="28"
                y2="4"
                stroke={item.color}
                strokeWidth={2}
                strokeDasharray={item.dash}
              />
            </svg>
            <span>{item.label}</span>
          </div>
        ))}
      </div>
    </Panel>
  );
}
