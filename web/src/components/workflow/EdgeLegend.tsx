import { Panel } from '@xyflow/react';
import { useT } from '../../i18n';
import { EDGE_COLORS, EDGE_DASH, PAUSE_DASH } from './flow';

export function EdgeLegend() {
  const t = useT();
  const items = [
    { label: t('edgeLegendNext'), color: EDGE_COLORS.forward, dash: EDGE_DASH.forward },
    { label: t('edgeLegendLoopReturn'), color: EDGE_COLORS.loop, dash: EDGE_DASH.loop },
    { label: t('edgeLegendExhausted'), color: EDGE_COLORS.exhausted, dash: EDGE_DASH.exhausted },
    { label: t('pause'), color: EDGE_COLORS.forward, dash: PAUSE_DASH }
  ];

  return (
    <Panel position="top-left">
      <div className="flex flex-col gap-1 rounded-md border border-border bg-panel/90 px-2 py-1.5 text-xs text-muted">
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
