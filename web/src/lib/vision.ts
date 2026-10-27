import type { ThreadItem } from './fold';

/** An image the model received: a tool result or an attachment of the human. */
export interface SeenImage {
  id: string;
  src: string;
  label: string;
  anchorId: string;
  inContext: boolean;
}

export function toolAnchorId(toolId: string): string {
  return `tool-${toolId}`;
}

export function attachmentAnchorId(attachmentId: string): string {
  return `attachment-${attachmentId}`;
}

export function seenImages(items: ThreadItem[]): SeenImage[] {
  // Only the last compaction matters: it is the point after which the model
  // no longer sees the earlier images.
  let lastCompaction = -1;
  for (let i = items.length - 1; i >= 0; i -= 1) {
    const item = items[i];
    if (item.kind === 'note' && item.id.startsWith('compacted-')) {
      lastCompaction = i;
      break;
    }
  }

  const images: SeenImage[] = [];
  items.forEach((item, index) => {
    const inContext = index > lastCompaction;

    if (item.kind === 'user' && !item.isSystemGenerated) {
      for (const attachment of item.attachments) {
        const anchorId = attachmentAnchorId(attachment.id);
        images.push({
          id: `${anchorId}-${images.length}`,
          src: attachment.data,
          label: attachment.filename,
          anchorId,
          inContext
        });
      }
    } else if (item.kind === 'assistant') {
      for (const segment of item.segments) {
        if (segment.kind !== 'tool' || !segment.tool.image) continue;
        const anchorId = toolAnchorId(segment.tool.id);
        images.push({
          id: `${anchorId}-${images.length}`,
          src: segment.tool.image.dataUrl,
          label: String(segment.tool.args.path ?? segment.tool.name),
          anchorId,
          inContext
        });
      }
    }
  });

  return images;
}
