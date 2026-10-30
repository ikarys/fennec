import { useState } from 'react';
import { panelBlock } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { Panel } from '../ui/Panel';

export function RequestSection({ request }: { request: string | null }) {
  const [expanded, setExpanded] = useState(false);

  if (request === null) {
    return null;
  }

  const expandable = request.length > 280 || request.split('\n').length > 4;

  return (
    <Panel id="request" title={panelBlock('request').title}>
      <p className={`m-0 whitespace-pre-wrap break-words text-xs text-text${expanded ? '' : ' line-clamp-4'}`}>
        {request}
      </p>
      {expandable && (
        <Button type="button" variant="ghost" size="sm" onClick={() => setExpanded((current) => !current)}>
          {expanded ? 'Replier' : 'Tout afficher'}
        </Button>
      )}
    </Panel>
  );
}
