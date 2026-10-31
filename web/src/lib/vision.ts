/** The DOM id of a tool card in the thread. */
export function toolAnchorId(toolId: string): string {
  return `tool-${toolId}`;
}

/** The DOM id of an attachment in the thread. */
export function attachmentAnchorId(attachmentId: string): string {
  return `attachment-${attachmentId}`;
}
