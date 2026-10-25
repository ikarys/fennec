export interface FennecEvent {
  seq?: number;
  type: string;
  data: any;
}

export type ToolStatus = 'running' | 'success' | 'error';

export interface ToolCallView {
  id: string;
  name: string;
  args: Record<string, unknown>;
  status: ToolStatus;
  durationMs?: number;
  stdout: string;
  stderr: string;
  output: string;
  error?: string;
  truncated?: boolean;
  exitCode?: number;
  addedLines?: number;
  removedLines?: number;
  contentPreview?: string[];
  diff?: {
    removed: string[];
    added: string[];
  };
}

export type Segment =
  | { kind: 'thinking'; id: string; content: string }
  | { kind: 'text'; id: string; content: string }
  | { kind: 'tool'; id: string; tool: ToolCallView };

export interface AssistantItem {
  kind: 'assistant';
  id: string;
  segments: Segment[];
}

export type ThreadItem =
  | { kind: 'user'; id: string; content: string; isSystemGenerated: boolean }
  | AssistantItem
  | { kind: 'error'; id: string; message: string }
  | { kind: 'note'; id: string; text: string };

export interface ModifiedFile {
  path: string;
  added: number;
  removed: number;
}

export interface ToolCount {
  name: string;
  count: number;
}

export interface ThreadView {
  items: ThreadItem[];
  contextTokens?: number;
  contextMax?: number;
  running: boolean;
  /** Reason of the last `chat.done`: the session state once a turn ends. */
  lastDone?: string;
  modifiedFiles: ModifiedFile[];
  toolCounts: ToolCount[];
}

function splitLines(value: string): string[] {
  return value.split(/\r?\n/);
}

function parseArgs(value: unknown): Record<string, unknown> {
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value);
      return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, unknown>) : {};
    } catch {
      return {};
    }
  }
  if (typeof value === 'object' && value !== null) {
    return value as Record<string, unknown>;
  }
  return {};
}

function parseExitCode(output: string): number | undefined {
  const match = output.match(/\[Exit code: (\d+)\]/);
  return match ? Number(match[1]) : undefined;
}

function updateToolStats(tool: ToolCallView, modified: Map<string, ModifiedFile>, toolCounts: Map<string, number>): void {
  toolCounts.set(tool.name, (toolCounts.get(tool.name) ?? 0) + 1);

  if (tool.name === 'write_file') {
    const path = String(tool.args.path ?? '');
    const content = String(tool.args.content ?? '');
    const lines = splitLines(content);
    tool.contentPreview = lines.slice(0, 20);
    tool.addedLines = lines.length;
    tool.removedLines = 0;
    if (path) {
      const entry = modified.get(path) ?? { path, added: 0, removed: 0 };
      entry.added += lines.length;
      modified.set(path, entry);
    }
  }

  if (tool.name === 'edit_file') {
    const path = String(tool.args.path ?? '');
    const oldLines = splitLines(String(tool.args.old_string ?? ''));
    const newLines = splitLines(String(tool.args.new_string ?? ''));
    tool.diff = { removed: oldLines, added: newLines };
    tool.addedLines = newLines.length;
    tool.removedLines = oldLines.length;
    if (path) {
      const entry = modified.get(path) ?? { path, added: 0, removed: 0 };
      entry.added += newLines.length;
      entry.removed += oldLines.length;
      modified.set(path, entry);
    }
  }

  if (tool.name === 'run_command') {
    tool.exitCode = parseExitCode(tool.output);
  }
}

function findAssistant(items: ThreadItem[], messageId: string): AssistantItem | undefined {
  return items.find((item): item is AssistantItem => item.kind === 'assistant' && item.id === messageId);
}

function findTool(items: ThreadItem[], toolCallId: string): ToolCallView | undefined {
  for (const item of items) {
    if (item.kind !== 'assistant') continue;
    for (const segment of item.segments) {
      if (segment.kind === 'tool' && segment.tool.id === toolCallId) {
        return segment.tool;
      }
    }
  }
  return undefined;
}

export function fold(events: FennecEvent[]): ThreadView {
  const items: ThreadItem[] = [];
  let contextTokens: number | undefined;
  let contextMax: number | undefined;
  let running = false;
  let lastDone: string | undefined;
  const modified = new Map<string, ModifiedFile>();
  const toolCounts = new Map<string, number>();

  events.forEach((event, index) => {
    const data = event.data ?? {};

    switch (event.type) {
      case 'message.start': {
        const messageId = String(data.messageId ?? `message-${index}`);
        if (data.role === 'user') {
          items.push({
            kind: 'user',
            id: messageId,
            content: String(data.content ?? ''),
            isSystemGenerated: Boolean(data.isSystemGenerated)
          });
        } else if (data.role === 'assistant') {
          const assistant: AssistantItem = {
            kind: 'assistant',
            id: messageId,
            segments: []
          };
          items.push(assistant);
          if (data.content) {
            assistant.segments.push({ kind: 'text', id: `${messageId}-text`, content: String(data.content) });
          }
        }
        break;
      }

      case 'message.delta': {
        const messageId = String(data.messageId ?? `message-${index}`);
        const assistant = findAssistant(items, messageId);
        if (!assistant) break;
        const content = String(data.content ?? '');
        const last = assistant.segments[assistant.segments.length - 1];
        if (last && last.kind === 'text') {
          last.content += content;
        } else {
          assistant.segments.push({ kind: 'text', id: `${messageId}-text-${index}`, content });
        }
        break;
      }

      case 'message.thinking': {
        const messageId = String(data.messageId ?? `message-${index}`);
        const assistant = findAssistant(items, messageId);
        if (!assistant) break;
        const content = String(data.content ?? '');
        const existing = assistant.segments.find((segment): segment is Extract<Segment, { kind: 'thinking' }> => segment.kind === 'thinking');
        if (existing) {
          existing.content += content;
        } else {
          assistant.segments.unshift({ kind: 'thinking', id: `${messageId}-thinking`, content });
        }
        break;
      }

      case 'tool.call': {
        const messageId = String(data.messageId ?? `message-${index}`);
        const toolCall = data.toolCall ?? {};
        const toolId = String(toolCall.id ?? `tool-${index}`);
        const name = String(toolCall.name ?? 'tool');
        const args = parseArgs(toolCall.arguments);
        const tool: ToolCallView = {
          id: toolId,
          name,
          args,
          status: 'running',
          stdout: '',
          stderr: '',
          output: ''
        };
        updateToolStats(tool, modified, toolCounts);

        let assistant = findAssistant(items, messageId);
        if (!assistant) {
          assistant = { kind: 'assistant', id: messageId, segments: [] };
          items.push(assistant);
        }
        assistant.segments.push({ kind: 'tool', id: toolId, tool });
        break;
      }

      case 'tool.output': {
        const toolCallId = String(data.toolCallId ?? '');
        const tool = findTool(items, toolCallId);
        if (!tool) break;
        const content = String(data.content ?? '');
        if (data.stream === 'stderr') {
          tool.stderr += content;
        } else {
          tool.stdout += content;
        }
        break;
      }

      case 'tool.result': {
        const toolCallId = String(data.toolCallId ?? '');
        const tool = findTool(items, toolCallId);
        if (!tool) break;
        const result = data.result ?? {};
        tool.status = result.success ? 'success' : 'error';
        tool.durationMs = typeof result.durationMs === 'number' ? result.durationMs : undefined;
        tool.output = String(result.output ?? '');
        tool.error = result.error ? String(result.error) : undefined;
        tool.truncated = Boolean(result.truncated);
        if (tool.name === 'run_command') {
          tool.exitCode = parseExitCode(tool.output);
        }
        break;
      }

      case 'context.state': {
        contextTokens = typeof data.currentTokens === 'number' ? data.currentTokens : contextTokens;
        contextMax = typeof data.maxTokens === 'number' ? data.maxTokens : contextMax;
        break;
      }

      case 'running.changed': {
        running = Boolean(data.isRunning);
        break;
      }

      case 'chat.done': {
        const reason = String(data.reason ?? 'complete');
        lastDone = reason;
        if (reason !== 'complete') {
          const text =
            reason === 'stopped' ? 'Arrêtée' : reason === 'truncated' ? 'Tronquée' : reason === 'error' ? 'Erreur' : 'Terminée';
          items.push({ kind: 'note', id: `done-${index}`, text });
        }
        break;
      }

      case 'chat.error': {
        items.push({
          kind: 'error',
          id: `error-${index}`,
          message: String(data.error ?? 'Erreur inconnue')
        });
        break;
      }

      default:
        break;
    }
  });

  return {
    items,
    contextTokens,
    contextMax,
    running,
    lastDone,
    modifiedFiles: Array.from(modified.values()),
    toolCounts: Array.from(toolCounts.entries()).map(([name, count]) => ({ name, count }))
  };
}
