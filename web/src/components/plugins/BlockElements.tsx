/**
 * The one renderer of a plugin block's typed elements: the core renders the
 * data, never HTML. Every placement uses it; `compact` is the status bar's
 * inline, small form. The tones map onto the primitives' existing colours.
 */
import { Fragment } from 'react';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Divider } from '../ui/Divider';
import type { PluginBlockElement, PluginTone } from '../../lib/api';

/** A plugin's tint, mapped to the primitives' tones: no colour of its own. */
export const TONE_TO_BADGE: Record<PluginTone, BadgeTone> = {
  none: 'neutral',
  info: 'info',
  success: 'ok',
  warning: 'attention',
  danger: 'err'
};

/** The progress bar's fill, by tone. */
const FILL: Record<PluginTone, string> = {
  none: 'bg-border-strong',
  info: 'bg-info',
  success: 'bg-ok',
  warning: 'bg-attention',
  danger: 'bg-err'
};

/** A list item's dot, by tone. */
const DOT: Record<PluginTone, string> = {
  none: 'bg-muted',
  info: 'bg-info',
  success: 'bg-ok',
  warning: 'bg-attention',
  danger: 'bg-err'
};

/** A compact figure's value text, by tone. */
const TONE_TEXT: Record<PluginTone, string> = {
  none: 'text-text',
  info: 'text-info',
  success: 'text-ok',
  warning: 'text-attention',
  danger: 'text-err'
};

/** The link's host, for its tooltip. */
function linkHost(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

export function BlockElements({ elements, compact = false }: { elements: PluginBlockElement[]; compact?: boolean }) {
  return (
    <div className={compact ? 'flex flex-wrap items-center gap-x-2.5 gap-y-0.5' : 'flex flex-col gap-1'}>
      {elements.map((element, index) => (
        <Fragment key={index}>
          {compact && index > 0 ? <Divider className="h-4" /> : null}
          <Element element={element} compact={compact} />
        </Fragment>
      ))}
    </div>
  );
}

function Element({ element, compact }: { element: PluginBlockElement; compact: boolean }) {
  switch (element.kind) {
    case 'text':
      return <span className="text-xs text-muted">{element.text}</span>;
    case 'figure':
      return (
        <span className={compact ? 'flex items-baseline gap-1' : 'flex items-baseline gap-2'}>
          <span
            className={`font-mono tabular-nums ${compact ? 'text-xs' : 'text-lg'} ${
              compact && element.tone && element.tone !== 'none' ? TONE_TEXT[element.tone] : 'text-text'
            }`}
          >
            {element.value}
          </span>
          <span className="text-xs text-muted">{element.label}</span>
          {!compact && element.tone && element.tone !== 'none' ? (
            <Badge tone={TONE_TO_BADGE[element.tone]} appearance="pill">
              {element.tone}
            </Badge>
          ) : null}
        </span>
      );
    case 'badge':
      return (
        <Badge tone={element.tone ? TONE_TO_BADGE[element.tone] : 'neutral'} appearance="pill">
          {element.label}
        </Badge>
      );
    case 'list':
      return (
        <ul className={compact ? 'flex flex-wrap items-center gap-x-2 gap-y-0.5' : 'flex flex-col gap-0.5'}>
          {element.items.map((item, index) => (
            <li key={index} className="flex items-center gap-1.5">
              <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${DOT[item.tone ?? 'none']}`} />
              <span className="text-xs text-text">{item.text}</span>
            </li>
          ))}
        </ul>
      );
    case 'progress': {
      const percent = Math.min(100, Math.max(0, (element.value / element.max) * 100));
      const fill = FILL[element.tone ?? 'none'];
      return compact ? (
        <span className="flex items-center gap-1.5">
          <span className="text-xs text-muted">{element.label}</span>
          <span className="h-1 w-10 shrink-0 overflow-hidden rounded-full bg-raised">
            <span className={`block h-full ${fill}`} style={{ width: `${percent}%` }} />
          </span>
          <span className="font-mono text-xs tabular-nums text-text">
            {element.value} / {element.max}
          </span>
        </span>
      ) : (
        <span className="flex flex-col gap-1">
          <span className="flex items-baseline justify-between gap-2">
            <span className="text-xs text-muted">{element.label}</span>
            <span className="font-mono text-xs tabular-nums text-text">
              {element.value} / {element.max}
            </span>
          </span>
          <span className="h-1.5 overflow-hidden rounded-full bg-raised">
            <span className={`block h-full ${fill}`} style={{ width: `${percent}%` }} />
          </span>
        </span>
      );
    }
    case 'link':
      return (
        <a
          href={element.url}
          target="_blank"
          rel="noopener noreferrer"
          title={linkHost(element.url)}
          className="text-xs text-accent-strong underline decoration-accent/40 underline-offset-2 hover:text-accent"
        >
          {element.label}
        </a>
      );
    case 'table':
      return (
        <table className="w-full border-collapse text-xs">
          <thead>
            <tr>
              {element.columns.map((column, index) => (
                <th key={index} className="border-b border-border px-1.5 py-1 text-left font-medium text-muted">
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {element.rows.map((row, rowIndex) => (
              <tr key={rowIndex} className={rowIndex % 2 === 1 ? 'bg-raised/40' : undefined}>
                {row.map((cell, cellIndex) => (
                  <td key={cellIndex} className="px-1.5 py-1 text-text">
                    {cell}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      );
  }
}
