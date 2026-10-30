import { formatTokens } from './format';
import type { NodeKind } from './workflow';
import { createRunFold, type RunView } from './workflowRun';

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
  image?: { dataUrl: string; mimeType: string };
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
  /** Tokens per second while the message streams, estimated by the server. */
  liveSpeed?: number;
}

export interface WorkflowNodeStat {
  nodeId: string;
  label: string;
  runs: number;
  workingMs: number;
}

export interface WorkflowStats {
  workingMs: number;
  nodes: WorkflowNodeStat[];
  reruns: number;
  filesChanged: number;
  criteriaTotal: number;
  criteriaPassed: number;
  criteriaVerified: number;
}

export type ThreadItem =
  | { kind: 'user'; id: string; content: string; isSystemGenerated: boolean; attachments: { id: string; filename: string; data: string }[] }
  | AssistantItem
  | { kind: 'mode'; id: string; name: string; color: string }
  | { kind: 'error'; id: string; message: string }
  | { kind: 'note'; id: string; text: string; detail?: string }
  | { kind: 'workflow-node'; id: string; nodeId: string; label: string; nodeKind: NodeKind; pass: number; port?: string; output?: string; exitCode?: number; childSessionId?: string; prompt?: string; durationMs?: number; startedAt?: number }
  | { kind: 'workflow-end'; id: string; runId: string; outcome: 'done' | 'blocked' | 'stopped' | 'error'; reason?: string; stats?: WorkflowStats };

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

export type Provenance =
  | { kind: 'verified'; command: string; exitCode?: number; output: string }
  | { kind: 'indicative'; agent: string; reason: string };

export interface CriterionView {
  id: string;
  description: string;
  check?: string;
  status: 'pending' | 'passed' | 'failed';
  provenance?: Provenance;
}

export interface DecisionView {
  callId: string;
  title: string;
  choice: string;
  reason: string;
  alternatives: string[];
}

export interface ThreadView {
  items: ThreadItem[];
  criteria: CriterionView[];
  decisions: DecisionView[];
  running: boolean;
  /** Reason of the last `chat.done`: the session state once a turn ends. */
  lastDone?: string;
  /** The unanswered `ask_user` question, if any. */
  pendingQuestion?: PendingQuestion;
  modifiedFiles: ModifiedFile[];
  toolCounts: ToolCount[];
  /**
   * The last `mode.changed` value, or null when the session detail should
   * provide the server's resolved mode.
   */
  mode: string | null;
  /** The latest workflow run, if any. */
  run?: RunView;
  /** The last generated session name, if any (last one wins). */
  name?: string;
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

function parseProvenance(value: unknown): Provenance | undefined {
  if (typeof value !== 'object' || value === null) return undefined;
  const provenance = value as Record<string, unknown>;
  if (provenance.kind === 'verified') {
    const command = typeof provenance.command === 'string' ? provenance.command : undefined;
    const output = typeof provenance.output === 'string' ? provenance.output : undefined;
    if (command === undefined || output === undefined) return undefined;
    return {
      kind: 'verified',
      command,
      exitCode: typeof provenance.exitCode === 'number' ? provenance.exitCode : undefined,
      output
    };
  }
  if (provenance.kind === 'indicative') {
    const agent = typeof provenance.agent === 'string' ? provenance.agent : undefined;
    const reason = typeof provenance.reason === 'string' ? provenance.reason : undefined;
    if (agent === undefined || reason === undefined) return undefined;
    return { kind: 'indicative', agent, reason };
  }
  return undefined;
}

function parseCriterionStatus(value: unknown): CriterionView['status'] | undefined {
  if (typeof value !== 'object' || value === null) return undefined;
  const status = value as Record<string, unknown>;
  if (status.type === 'pending') return 'pending';
  if (status.type === 'passed' || status.type === 'failed') return status.type;
  return undefined;
}

function parseCriteria(value: unknown): CriterionView[] | null {
  if (!Array.isArray(value)) return null;
  let criteria: CriterionView[] = [];
  for (const raw of value) {
    if (typeof raw !== 'object' || raw === null) continue;
    const criterion = raw as Record<string, unknown>;
    const id = typeof criterion.id === 'string' ? criterion.id : undefined;
    const description = typeof criterion.description === 'string' ? criterion.description : undefined;
    if (id === undefined || description === undefined) continue;
    const check = typeof criterion.check === 'string' ? criterion.check : undefined;
    criteria.push({ id, description, check, status: 'pending' });
  }
  return criteria;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function parseWorkflowStats(value: unknown): WorkflowStats | undefined {
  if (typeof value !== 'object' || value === null) return undefined;
  const stats = value as Record<string, unknown>;
  const rawNodes = Array.isArray(stats.nodes) ? stats.nodes : [];
  const nodes: WorkflowNodeStat[] = [];
  for (const raw of rawNodes) {
    if (typeof raw !== 'object' || raw === null) continue;
    const node = raw as Record<string, unknown>;
    const nodeId = typeof node.nodeId === 'string' ? node.nodeId : undefined;
    const label = typeof node.label === 'string' ? node.label : undefined;
    if (nodeId === undefined || label === undefined) continue;
    nodes.push({
      nodeId,
      label,
      runs: isFiniteNumber(node.runs) ? node.runs : 0,
      workingMs: isFiniteNumber(node.workingMs) ? node.workingMs : 0
    });
  }
  return {
    workingMs: isFiniteNumber(stats.workingMs) ? stats.workingMs : 0,
    nodes,
    reruns: isFiniteNumber(stats.reruns) ? stats.reruns : 0,
    filesChanged: isFiniteNumber(stats.filesChanged) ? stats.filesChanged : 0,
    criteriaTotal: isFiniteNumber(stats.criteriaTotal) ? stats.criteriaTotal : 0,
    criteriaPassed: isFiniteNumber(stats.criteriaPassed) ? stats.criteriaPassed : 0,
    criteriaVerified: isFiniteNumber(stats.criteriaVerified) ? stats.criteriaVerified : 0
  };
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
  /**
   * The events of a run older than the loaded page: they feed the run,
   * not the thread.
   */
  applyRunOnly(event: FennecEvent): void;
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
  let running = false;
  let lastDone: string | undefined;
  let pendingQuestion: PendingQuestion | undefined;
  const modified = new Map<string, ModifiedFile>();
  const toolCounts = new Map<string, number>();
  const assistants = new Map<string, AssistantItem>();
  const tools = new Map<string, ToolCallView>();
  let index = 0;
  // Tokens generated since the last `chat.done`, for the turn recap.
  // Null means "no live change yet": the session detail owns the mode.
  let mode: string | null = null;
  let criteria: CriterionView[] = [];
  const decisions: DecisionView[] = [];
  let name: string | undefined;
  const runFold = createRunFold();
  type WorkflowNodeItem = Extract<ThreadItem, { kind: 'workflow-node' }>;
  const openWorkflowItems = new Map<string, WorkflowNodeItem>();
  let pendingPromptFor: WorkflowNodeItem | null = null;

  const pushItem = (item: ThreadItem) => {
    // The node's prompt is the last of fennec's messages after its divider:
    // an agent's mode reminder may come first.
    if (pendingPromptFor) {
      if (item.kind === 'user' && item.isSystemGenerated) {
        pendingPromptFor.prompt = item.content;
      } else {
        pendingPromptFor = null;
      }
    }
    items.push(item);
  };

  const humanWords = (id: string, content: string) =>
    pushItem({ kind: 'user', id, content, isSystemGenerated: false, attachments: [] });

  const newAssistant = (id: string): AssistantItem => {
    const assistant: AssistantItem = { kind: 'assistant', id, segments: [], done: false };
    pushItem(assistant);
    assistants.set(id, assistant);
    return assistant;
  };

  // The last estimate stays until the message ends: deltas flushed in the
  // first half second, or replayed from the log, carry none.
  const streamed = (assistant: AssistantItem, data: Record<string, unknown>) => {
    if (typeof data.tokensPerSecond === 'number') assistant.liveSpeed = data.tokensPerSecond;
  };

  const apply = (event: FennecEvent) => {
    index += 1;
    const data = event.data ?? {};
    runFold.apply(event);

    switch (event.type) {
      case 'message.start': {
        const messageId = String(data.messageId ?? `message-${index}`);
        const metadata = data.metadata as { type?: string; name?: string; color?: string } | undefined;

        // An agent reminder is a mode marker, not a user message: its content
        // is the agent prompt and must not be rendered as a bubble.
        if (metadata?.type === 'agent') {
          pushItem({
            kind: 'mode',
            id: messageId,
            name: String(metadata.name ?? ''),
            color: String(metadata.color ?? '')
          });
          break;
        }

        if (data.role === 'user') {
          const rawAttachments = Array.isArray(data.attachments) ? data.attachments : [];
          pushItem({
            kind: 'user',
            id: messageId,
            content: String(data.content ?? ''),
            isSystemGenerated: Boolean(data.isSystemGenerated),
            attachments: rawAttachments.map((a: any) => ({
              id: String(a?.id ?? ''),
              filename: String(a?.filename ?? ''),
              data: String(a?.data ?? '')
            }))
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
        streamed(assistant, data);
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
        streamed(assistant, data);
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
        const metadata = result.metadata as { dataUrl?: string; mimeType?: string } | undefined;
        if (typeof metadata?.dataUrl === 'string' && typeof metadata?.mimeType === 'string' && metadata.mimeType.startsWith('image/')) {
          tool.image = { dataUrl: metadata.dataUrl, mimeType: metadata.mimeType };
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

      case 'context.compacted': {
        const beforeTokens = typeof data.beforeTokens === 'number' ? data.beforeTokens : 0;
        const afterTokens = typeof data.afterTokens === 'number' ? data.afterTokens : 0;
        const summary = String(data.summary ?? '');
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
        pushItem({
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

      case 'mode.changed': {
        // The live event wins over the session detail until the next change.
        if (typeof data.mode === 'string') {
          mode = data.mode;
        }
        break;
      }

      case 'criteria.set': {
        const parsed = parseCriteria(data.criteria);
        if (parsed) {
          criteria = parsed;
        }
        break;
      }

      case 'decision.recorded': {
        const text = (value: unknown) => (typeof value === 'string' ? value : '');
        const title = text(data.title);
        if (!title) break;
        decisions.push({
          callId: text(data.callId),
          title,
          choice: text(data.choice),
          reason: text(data.reason),
          alternatives: Array.isArray(data.alternatives) ? data.alternatives.filter((a: unknown) => typeof a === 'string') : []
        });
        break;
      }

      case 'criterion.updated': {
        const criterionId = String(data.criterionId ?? '');
        const status = parseCriterionStatus(data.status);
        if (!criterionId || !status) break;
        const criterion = criteria.find((item) => item.id === criterionId);
        if (!criterion) break;
        criterion.status = status;
        if (status === 'pending') {
          criterion.provenance = undefined;
        } else {
          criterion.provenance = parseProvenance(data.status?.provenance);
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
          pushItem({ kind: 'note', id: `done-${index}`, text });
        }
        break;
      }

      case 'chat.error': {
        pushItem({
          kind: 'error',
          id: `error-${index}`,
          message: String(data.error ?? 'Erreur inconnue')
        });
        break;
      }

      case 'integration.applied': {
        const commits = Array.isArray(data.commits) ? data.commits.map((commit: unknown) => String(commit)) : [];
        const head = String(data.head ?? '');
        const acknowledged = Array.isArray(data.acknowledged) ? data.acknowledged.map((kind: unknown) => String(kind)) : [];
        const detailParts: string[] = [];
        if (commits.length > 0) {
          detailParts.push(commits.join(' '));
        }
        if (acknowledged.length > 0) {
          detailParts.push(`Accepté malgré : ${acknowledged.join(', ')}`);
        }
        pushItem({
          kind: 'note',
          id: `integration-applied-${index}`,
          text: `Intégré par toi · ${commits.length} commit${commits.length === 1 ? '' : 's'} dans HEAD ${head.slice(0, 8)}`,
          detail: detailParts.length > 0 ? detailParts.join('\n') : undefined
        });
        break;
      }

      case 'worktree.discarded': {
        const commits = Array.isArray(data.commits) ? data.commits.map((commit: unknown) => String(commit)) : [];
        pushItem({
          kind: 'note',
          id: `worktree-discarded-${index}`,
          text: `Travail de l'agent abandonné · ${commits.length} commit${commits.length === 1 ? '' : 's'}`,
          detail: commits.length > 0 ? commits.join(' ') : undefined
        });
        break;
      }

      case 'session.rewound': {
        const messageId = String(data.messageId ?? '');
        const index = items.findIndex((item) => item.id === messageId);
        if (index >= 0) {
          items.splice(index);
        } else {
          items.length = 0;
        }
        if (pendingQuestion) {
          const callId = pendingQuestion.callId;
          const remains = items.some(
            (item) =>
              item.kind === 'assistant' &&
              item.segments.some((segment) => segment.kind === 'tool' && segment.tool.id === callId)
          );
          if (!remains) {
            pendingQuestion = undefined;
          }
        }
        break;
      }

      case 'session.name_generated': {
        if (typeof data.name === 'string') {
          name = data.name;
        }
        break;
      }

      case 'workflow.started': {
        const workflow = runFold.view()?.workflow;
        if (workflow) {
          pushItem({ kind: 'note', id: `workflow-started-${index}`, text: `Workflow « ${workflow.name} »` });
        }
        // The task is what the human typed: their message, not a detail of the note.
        if (data.task) humanWords(`workflow-task-${index}`, String(data.task));
        break;
      }

      case 'workflow.resumed': {
        if (data.comment) humanWords(`workflow-resumed-${index}`, String(data.comment));
        break;
      }

      case 'workflow.node.started': {
        const runId = String(data.runId ?? '');
        const nodeId = String(data.nodeId ?? '');
        const pass = typeof data.pass === 'number' ? data.pass : 1;
        const node = runFold.view()?.workflow.nodes.find((n) => n.id === nodeId);
        const kind = node?.kind ?? 'agent';
        if (kind === 'parallel' || kind === 'join' || kind === 'start') {
          break;
        }
        const item: WorkflowNodeItem = {
          kind: 'workflow-node',
          id: `wf-${runId}-${nodeId}-${pass}`,
          nodeId,
          label: node?.name ?? nodeId,
          nodeKind: kind,
          pass
        };
        if (typeof data.childSessionId === 'string') {
          item.childSessionId = data.childSessionId;
        }
        if (typeof data.startedAt === 'number') {
          item.startedAt = data.startedAt;
        }
        // A resume restarts an interrupted node on its own pass: the restart
        // replaces the unfinished divider instead of showing a second one.
        const stale = openWorkflowItems.get(`${runId}:${nodeId}`);
        if (stale && stale.pass === pass) {
          items.splice(items.indexOf(stale), 1);
        }
        pushItem(item);
        openWorkflowItems.set(`${runId}:${nodeId}`, item);
        if (kind === 'agent') {
          pendingPromptFor = item;
        }
        break;
      }

      case 'workflow.node.finished': {
        const runId = String(data.runId ?? '');
        const nodeId = String(data.nodeId ?? '');
        const item = openWorkflowItems.get(`${runId}:${nodeId}`);
        if (item) {
          item.port = String(data.port ?? '');
          item.output = String(data.output ?? '');
          if (typeof data.exitCode === 'number') {
            item.exitCode = data.exitCode;
          }
          if (typeof data.durationMs === 'number') {
            item.durationMs = data.durationMs;
          }
          openWorkflowItems.delete(`${runId}:${nodeId}`);
        }
        break;
      }

      case 'workflow.finished': {
        const outcome = (data.outcome as 'done' | 'blocked' | 'stopped' | 'error') ?? 'error';
        pushItem({
          kind: 'workflow-end',
          id: `workflow-finished-${index}`,
          runId: String(data.runId ?? ''),
          outcome,
          reason: typeof data.reason === 'string' ? data.reason : undefined,
          stats: parseWorkflowStats(data.stats)
        });
        break;
      }

      default:
        break;
    }
  };

  const applyRunOnly = (event: FennecEvent): void => {
    runFold.apply(event);
  };

  const view = (): ThreadView => ({
    items: [...items],
    criteria: [...criteria],
    decisions: [...decisions],
    running,
    lastDone,
    pendingQuestion,
    modifiedFiles: Array.from(modified.values()),
    toolCounts: Array.from(toolCounts.entries()).map(([name, count]) => ({ name, count })),
    mode,
    name,
    run: runFold.view()
  });

  return { apply, view, applyRunOnly };
}
