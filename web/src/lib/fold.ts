import type { MessageStats } from './api';
import { formatTokens } from './format';

export interface FennecEvent {
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
  /** The question asked by `ask_user`, shown instead of raw arguments. */
  question?: string;
}

export type Segment =
  | { kind: 'thinking'; id: string; content: string }
  | { kind: 'text'; id: string; content: string }
  | { kind: 'tool'; id: string; tool: ToolCallView };

export interface AssistantItem {
  kind: 'assistant';
  id: string;
  segments: Segment[];
  done: boolean;
  /** Measured by the server once the message is done. */
  stats?: MessageStats;
  /** Characters streamed (text and reasoning), for the live estimate. */
  chars: number;
  /** Estimated tokens per second while the message streams. */
  liveSpeed?: number;
}

/** What the session's model calls cost so far. */
export interface SessionUsage {
  calls: number;
  prefillTokens: number;
  cachedPromptTokens: number;
  generationTokens: number;
  last?: MessageStats;
}

export type ThreadItem =
  | { kind: 'user'; id: string; content: string; isSystemGenerated: boolean }
  | AssistantItem
  | { kind: 'error'; id: string; message: string }
  | { kind: 'note'; id: string; text: string; detail?: string };

export interface ModifiedFile {
  path: string;
  added: number;
  removed: number;
}

export interface ToolCount {
  name: string;
  count: number;
}

export interface PendingQuestion {
  callId: string;
  question: string;
  type: 'text' | 'confirm' | 'choice';
  options: {
    value: string;
    label: string;
    description?: string;
  }[];
}

export interface ThreadView {
  items: ThreadItem[];
  contextTokens?: number;
  contextMax?: number;
  running: boolean;
  /** Reason of the last `chat.done`: the session state once a turn ends. */
  lastDone?: string;
  /** The unanswered `ask_user` question, if any. */
  pendingQuestion?: PendingQuestion;
  modifiedFiles: ModifiedFile[];
  toolCounts: ToolCount[];
  usage: SessionUsage;
}

// The live speed is estimated from characters over the last few seconds;
// the ratio is learnt from the messages the server measured.
const LIVE_WINDOW_MS = 3000;
const DEFAULT_TOKENS_PER_CHAR = 0.28;

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

export interface ThreadFold {
  apply(event: FennecEvent): void;
  /** A new view object per call, for React; the items inside are shared. */
  view(): ThreadView;
}

/**
 * Builds the thread view event by event. The raw events are not kept, and
 * messages and tool calls are found through indexes: a long session costs
 * memory for what is shown, and each event a constant amount of work.
 */
export function createThreadFold(): ThreadFold {
  const items: ThreadItem[] = [];
  let contextTokens: number | undefined;
  let contextMax: number | undefined;
  let running = false;
  let lastDone: string | undefined;
  let pendingQuestion: PendingQuestion | undefined;
  const modified = new Map<string, ModifiedFile>();
  const toolCounts = new Map<string, number>();
  const assistants = new Map<string, AssistantItem>();
  const tools = new Map<string, ToolCallView>();
  const usage: SessionUsage = { calls: 0, prefillTokens: 0, cachedPromptTokens: 0, generationTokens: 0 };
  let tokensPerChar = DEFAULT_TOKENS_PER_CHAR;
  // [arrival time, chars] of the streaming message, oldest first.
  let samples: [number, number][] = [];
  let index = 0;

  const newAssistant = (id: string): AssistantItem => {
    const assistant: AssistantItem = { kind: 'assistant', id, segments: [], done: false, chars: 0 };
    items.push(assistant);
    assistants.set(id, assistant);
    samples = [];
    return assistant;
  };

  const streamed = (assistant: AssistantItem, content: string) => {
    assistant.chars += content.length;
    const now = performance.now();
    samples.push([now, assistant.chars]);
    while (samples.length > 2 && now - samples[0][0] > LIVE_WINDOW_MS) samples.shift();
    const [t0, c0] = samples[0];
    // A replayed burst arrives at once: no speed until there is a span.
    if (now - t0 >= 500) {
      assistant.liveSpeed = ((assistant.chars - c0) * tokensPerChar) / ((now - t0) / 1000);
    }
  };

  const apply = (event: FennecEvent) => {
    index += 1;
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
          const assistant = newAssistant(messageId);
          if (data.content) {
            assistant.segments.push({ kind: 'text', id: `${messageId}-text`, content: String(data.content) });
          }
        }
        break;
      }

      case 'message.delta': {
        const messageId = String(data.messageId ?? `message-${index}`);
        const assistant = assistants.get(messageId);
        if (!assistant) break;
        const content = String(data.content ?? '');
        streamed(assistant, content);
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
        const assistant = assistants.get(messageId);
        if (!assistant) break;
        const content = String(data.content ?? '');
        streamed(assistant, content);
        const existing = assistant.segments.find((segment): segment is Extract<Segment, { kind: 'thinking' }> => segment.kind === 'thinking');
        if (existing) {
          existing.content += content;
        } else {
          assistant.segments.unshift({ kind: 'thinking', id: `${messageId}-thinking`, content });
        }
        break;
      }

      case 'message.done': {
        const assistant = assistants.get(String(data.messageId ?? ''));
        if (!assistant) break;
        assistant.done = true;
        assistant.liveSpeed = undefined;
        const stats = data.stats as MessageStats | undefined;
        if (!stats) break;
        assistant.stats = stats;
        usage.calls += 1;
        usage.prefillTokens += stats.prefillTokens;
        usage.cachedPromptTokens += stats.cachedPromptTokens;
        usage.generationTokens += stats.generationTokens;
        usage.last = stats;
        if (assistant.chars > 200 && stats.generationTokens > 0) {
          tokensPerChar = stats.generationTokens / assistant.chars;
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
        if (name === 'ask_user' && typeof args.question === 'string') {
          tool.question = args.question;
        }
        updateToolStats(tool, modified, toolCounts);

        const assistant = assistants.get(messageId) ?? newAssistant(messageId);
        assistant.segments.push({ kind: 'tool', id: toolId, tool });
        tools.set(toolId, tool);
        break;
      }

      case 'tool.output': {
        const toolCallId = String(data.toolCallId ?? '');
        const tool = tools.get(toolCallId);
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
        // The answer ends the pending question even if the tool view is missing.
        if (pendingQuestion?.callId === toolCallId) {
          pendingQuestion = undefined;
        }
        const tool = tools.get(toolCallId);
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

      case 'chat.ask_user': {
        const callId = String(data.callId ?? `tool-${index}`);
        const question = String(data.question ?? '');
        const type: PendingQuestion['type'] =
          data.type === 'confirm' ? 'confirm' : data.type === 'choice' ? 'choice' : 'text';
        const rawOptions = Array.isArray(data.options) ? data.options : [];
        const options: PendingQuestion['options'] = rawOptions.map((option: any) => {
          const value = String(option?.value ?? '');
          return {
            value,
            label: String(option?.label ?? value),
            description: option?.description != null ? String(option.description) : undefined
          };
        });
        pendingQuestion = { callId, question, type, options };
        const tool = tools.get(callId);
        if (tool && tool.name === 'ask_user') {
          tool.question = question;
        }
        break;
      }

      case 'context.state': {
        contextTokens = typeof data.currentTokens === 'number' ? data.currentTokens : contextTokens;
        contextMax = typeof data.maxTokens === 'number' ? data.maxTokens : contextMax;
        break;
      }

      case 'context.compacted': {
        const beforeTokens = typeof data.beforeTokens === 'number' ? data.beforeTokens : 0;
        const afterTokens = typeof data.afterTokens === 'number' ? data.afterTokens : 0;
        const summary = String(data.summary ?? '');
        contextTokens = afterTokens;
        const summaryTrimmed = summary.trim();
        for (let i = items.length - 1; i >= 0; i -= 1) {
          const item = items[i];
          if (item.kind !== 'assistant') continue;
          const text = item.segments
            .filter((segment): segment is Extract<Segment, { kind: 'text' }> => segment.kind === 'text')
            .map((segment) => segment.content)
            .join('')
            .trim();
          if (text === summaryTrimmed) {
            items.splice(i, 1);
            break;
          }
        }
        items.push({
          kind: 'note',
          id: `compacted-${index}`,
          text: `Contexte compacté · ${formatTokens(beforeTokens)} → ~${formatTokens(afterTokens)} tokens`,
          detail: summary
        });
        break;
      }

      case 'running.changed': {
        running = Boolean(data.isRunning);
        // A new turn supersedes any unanswered question.
        if (running) {
          pendingQuestion = undefined;
        }
        break;
      }

      case 'chat.done': {
        const reason = String(data.reason ?? 'complete');
        lastDone = reason;
        // The question card is the signal for `waiting_for_user`; no note is needed.
        if (reason !== 'complete' && reason !== 'waiting_for_user') {
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
  };

  const view = (): ThreadView => ({
    items: [...items],
    contextTokens,
    contextMax,
    running,
    lastDone,
    pendingQuestion,
    modifiedFiles: Array.from(modified.values()),
    toolCounts: Array.from(toolCounts.entries()).map(([name, count]) => ({ name, count })),
    usage: { ...usage }
  });

  return { apply, view };
}
