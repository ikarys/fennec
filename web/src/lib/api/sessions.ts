import { request } from "./request";
import type { ModelRef, SessionModel } from "./settings";

export type SessionState =
  | "running"
  | "complete"
  | "stopped"
  | "error"
  | "truncated"
  | "idle"
  | "waiting_for_user";

export interface BudgetNotice {
  projectId: string;
  project: string;
  status: 'alert' | 'exceeded';
  detail: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  state: SessionState;
  createdAt: number;
  updatedAt: number;
  parentId: string | null;
  /** The agent the session resolves to (analyze, audit-*, …), or null when it has none. */
  mode: string | null;
  /** A workflow ended done and nothing integrated or discarded its work since. */
  awaitingIntegration: boolean;
}

/** What a session is waiting for; the badge and the notification are worded from it. */
export type AttentionKind =
  | "question"
  | "workflowPause"
  | "awaitingIntegration"
  | "conflict"
  | "error";

/** One session that needs the human, as the `/api/notices` snapshot carries it. */
export interface AttentionItem {
  sessionId: string;
  projectId: string;
  title: string;
  kind: AttentionKind;
  /** The question text, the workflow's question, or the error message; empty for work ready to integrate. */
  detail: string;
}

export interface SessionDetail {
  id: string;
  title: string;
  state: SessionState;
  /** When the session was created, Unix ms; the server always sends it. */
  createdAt: number;
  /** The session's model, resolved by the server. */
  model: SessionModel;
  /** The agent id the session resolves to, or null when it has none. */
  mode: string | null;
  /** The parent session id, or null for a top-level session. */
  parentId: string | null;
  /** The git worktree the session's agent works in, or null when it runs in place. */
  worktree: {
    path: string;
    branch: string;
    base: string;
    /** Opens the copy in VS Code (through the WSL remote under WSL). */ editorUrl: string;
    /** The agent's commits past the base: 0 when there is nothing to integrate. */
    commits: number;
  } | null;
  project: {
    id: string;
    name: string;
    workdir: string;
    /** The scratch project behind the chat: no folder, no git. */
    chat?: boolean;
  };
  /** The idea this session frames, or null. */
  idea: { id: string; state: "inbox" | "framing" | "ready" } | null;
  /** The first message the human typed, verbatim, or null. */
  request: string | null;
  /** The last answer read as an audit report, or null. */
  audit: Audit | null;
  /** The tasks agents proposed in this session, in order. */
  proposedTasks: ProposedTask[];
  /** The last answer holds an AGENTS.md draft: whether the file exists, and whether the human already wrote this draft. */
  agentsMdDraft: { exists: boolean; written: boolean } | null;
  /** The last answer proposes architecture rules: the lines to append, and whether the human already added them. */
  ruleProposal: { lines: string[]; added: boolean } | null;
  /** The agent switch suggested in this turn, if the target agent still exists. */
  agentSuggestion: AgentSuggestion | null;
}

export interface AuditFinding {
  number: number;
  title: string;
  severity: 'high' | 'medium' | 'low';
  principle: string;
  locations: string[];
  problem: string;
  fix: string;
}

export interface Audit {
  scope: string;
  summary: string;
  findings: AuditFinding[];
  taken: number[];
}

/** A backlog task an agent proposed with propose_task; ideaId is set once the human accepted it. */
/** An agent's suggestion to go on in a new session with another agent (indicative). */
export interface AgentSuggestion {
  callId: string;
  agent: string;
  name: string;
  color: string | null;
  reason: string;
  brief: string;
}

export interface ProposedTask {
  callId: string;
  title: string;
  text: string;
  ideaId: string | null;
}

/** Turns the checked findings of a session's audit into backlog ideas. */
export function createAuditTasks(sessionId: string, findings: number[]): Promise<{ created: number }> {
  return request<{ created: number }>(`/api/sessions/${encodeURIComponent(sessionId)}/audit/tasks`, {
    method: 'POST',
    body: JSON.stringify({ findings }),
  });
}

/** Accepts a proposed task, turning it into a backlog idea. */
export function acceptProposedTask(sessionId: string, callId: string): Promise<{ ideaId: string }> {
  return request<{ ideaId: string }>(`/api/sessions/${encodeURIComponent(sessionId)}/proposals/${encodeURIComponent(callId)}/accept`, {
    method: 'POST',
    body: JSON.stringify({}),
  });
}

export function writeAgentsMd(sessionId: string): Promise<{ replaced: boolean }> {
  return request<{ replaced: boolean }>(`/api/sessions/${encodeURIComponent(sessionId)}/agents-md`, {
    method: 'POST',
    body: JSON.stringify({}),
  });
}

export function addArchitectureRules(sessionId: string): Promise<{ count: number }> {
  return request<{ count: number }>(`/api/sessions/${encodeURIComponent(sessionId)}/architecture-rules`, {
    method: 'POST',
    body: JSON.stringify({}),
  });
}

export function getSession(id: string): Promise<SessionDetail> {
  return request<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`);
}

/** `message.done` stats of one model call (`MessageStats` subset). */
export interface MessageStats {
  model: string;
  /** Seconds. */
  totalTime: number;
  prefillTokens: number;
  cachedPromptTokens: number;
  generationTokens: number;
  /** New prompt tokens per second. */
  prefillSpeed: number;
  generationSpeed: number;
}

/** A session's skills folded from its log: loaded so far, and still in the model's view. */
export interface SessionSkills {
  loaded: string[];
  active: string[];
  /** Frozen into the session's system prompt at its first turn. */
  alwaysOn: string[];
  /** Chosen always-on in Settings after this session's prompt froze: absent until typed as /<id>. */
  missedAlwaysOn: string[];
}

export function getSkills(id: string): Promise<SessionSkills> {
  return request<SessionSkills>(
    `/api/sessions/${encodeURIComponent(id)}/skills`,
  );
}

/** A session's model calls from the server's usage ledger: every figure is computed there. */
export interface SessionUsage {
  calls: number;
  promptTokens: number;
  cachedTokens: number;
  completionTokens: number;
  /** 0 to 100. */
  cachePercent: number;
  /** Estimated cost at each model's price, "—" when no model has one. */
  cost: string;
  /** How the cost was reached, or why there is none. */
  costDetail: string;
  /** The cost of all the prompt tokens, cached ones included; null when no model has a price. */
  inputCost: string | null;
  /** The share of the cost of cached tokens; null when no model has a price. */
  cachedCost: string | null;
  /** The share of the cost of generated tokens; null when no model has a price. */
  outputCost: string | null;
  /** Tokens per second of the last call; null before any call. */
  lastGenerationSpeed: number | null;
}

export function getUsage(id: string): Promise<SessionUsage> {
  return request<SessionUsage>(`/api/sessions/${encodeURIComponent(id)}/usage`);
}

/** The session header's context gauge, computed by the server. */
export interface ContextView {
  /** 0 to 100, the bar's width. */
  percent: number;
  tone: "ok" | "warn" | "err";
  /** Where the compaction threshold sits, 0 to 100; null when compaction is off. */
  compactMark: number | null;
  line: string;
  /** Taken, left, total, threshold. */
  title: string;
  /** Null while nothing is measured. */
  composition: ContextComposition | null;
}

export type ContextPartKind = "prompt" | "conversation" | "tools";

/** Indicative: the exact total spread over the parts by their share of the text. */
export interface ContextComposition {
  kinds: {
    kind: ContextPartKind;
    label: string;
    tokens: string;
    percent: number;
  }[];
  /** `kind` null: the row that sums the smallest parts. */
  parts: {
    kind: ContextPartKind | null;
    label: string;
    tokens: string;
    percent: number;
  }[];
  images: string | null;
}

export function getContext(id: string): Promise<ContextView> {
  return request<ContextView>(
    `/api/sessions/${encodeURIComponent(id)}/context`,
  );
}

/** The images the model received, computed by the server over the whole log. */
export interface SeenImage {
  label: string;
  anchorId: string;
  src: string;
  inContext: boolean;
}

export function getVision(id: string): Promise<SeenImage[]> {
  return request<SeenImage[]>(
    `/api/sessions/${encodeURIComponent(id)}/vision`,
  );
}

export interface PendingApproval {
  callId: string;
  subject: string;
  kind: 'command' | 'tool' | 'path';
  pattern: string;
  access: 'read' | 'write' | 'readWrite' | null;
  /** The folders the human may grant instead of `pattern`, narrowest first. */
  folders: string[];
}

export interface PendingWaiver {
  callId: string;
  path: string;
  reason: string;
}

export interface QueuedMessage {
  queueId: string;
  content: string;
}

/** How a criterion's status was established. */
export type Provenance =
  | { kind: "verified"; command: string; exitCode?: number; output: string }
  | { kind: "indicative"; agent: string; reason: string };

export type CriterionStatus =
  | { type: "pending" }
  | { type: "passed" | "failed"; provenance: Provenance };

export interface CriterionView {
  id: string;
  description: string;
  check?: string;
  status: CriterionStatus;
  /** Its check passed before any change (last preflight, verified). */
  passedBefore: boolean;
  /** Its check only searches the code: it proves the code exists, not that it works (verified). */
  existenceOnly: boolean;
}

/** A choice an agent recorded with record_decision: indicative. */
export interface DecisionView {
  callId: string;
  title: string;
  choice: string;
  reason: string;
  alternatives: string[];
}

export interface CriteriaTally {
  total: number;
  passed: number;
  failed: number;
  indicative: number;
}

/** The session's blocks folded by the server: the UI computes none. */
export interface ServedThread {
  pendingApproval: PendingApproval | null;
  pendingWaiver: PendingWaiver | null;
  /** Messages queued while a turn runs, in order. */
  queued: QueuedMessage[];
  /** The criteria in force: the last criteria.set and the updates since. */
  criteria: CriterionView[];
  /** How the criteria stand, counted by the server; `indicative` counts the passes an agent judged. */
  criteriaTally: CriteriaTally;
  decisions: DecisionView[];
  /** The ids of the messages the human may replay: typed, in the model's current view. */
  replayable: string[];
}

export function getThread(id: string): Promise<ServedThread> {
  return request<ServedThread>(
    `/api/sessions/${encodeURIComponent(id)}/thread`,
  );
}

export interface AttachmentInput {
  filename: string;
  data: string;
}

export function createSession(
  projectId: string,
  prompt: string,
  mode?: string,
  attachments?: AttachmentInput[],
  workflow?: { workflowId: string; stepByStep: boolean; worktree: boolean },
  model?: ModelRef,
  mcpDisabled?: string[],
): Promise<{ id: string }> {
  // `mode` is optional: omit it so the server applies its default resolution.
  const body: {
    projectId: string;
    prompt: string;
    mode?: string;
    attachments?: AttachmentInput[];
    workflowId?: string;
    stepByStep?: boolean;
    worktree?: boolean;
    model?: ModelRef;
    mcpDisabled?: string[];
  } = { projectId, prompt };
  if (mode != null) {
    body.mode = mode;
  }
  if (attachments && attachments.length > 0) {
    body.attachments = attachments;
  }
  if (workflow) {
    body.workflowId = workflow.workflowId;
    body.stepByStep = workflow.stepByStep;
    body.worktree = workflow.worktree;
  }
  if (model) {
    body.model = model;
  }
  if (mcpDisabled !== undefined) {
    body.mcpDisabled = mcpDisabled;
  }
  return request<{ id: string }>("/api/sessions", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function sendMessage(
  id: string,
  content: string,
  attachments?: AttachmentInput[],
): Promise<void> {
  const body: { content: string; attachments?: AttachmentInput[] } = {
    content,
  };
  if (attachments && attachments.length > 0) {
    body.attachments = attachments;
  }
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/messages`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function replayMessage(id: string, messageId: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/replay`, {
    method: "POST",
    body: JSON.stringify({ messageId }),
  });
}

export type ApprovalChoice = 'once' | 'session' | 'always' | 'never';

/** `pattern`: a folder from the request's `folders`, broader than the proposed one. */
export function answerRuleApproval(
  id: string,
  callId: string,
  choice: ApprovalChoice,
  pattern?: string
): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/rule-approval`, {
    method: "POST",
    body: JSON.stringify({ callId, choice, pattern }),
  });
}

export function answerWaiver(id: string, callId: string, hours: number | null): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/waiver`, {
    method: "POST",
    body: JSON.stringify(hours === null ? { callId, grant: false } : { callId, grant: true, hours }),
  });
}

export function cancelQueued(id: string, queueId: string): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(id)}/queue/${encodeURIComponent(queueId)}/cancel`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
  );
}

/** Re-runs a turn that ended in an error, with a continuation prompt. */
export function retrySession(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/retry`, {
    method: "POST",
  });
}

export function setSessionMode(id: string, mode: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/mode`, {
    method: "POST",
    body: JSON.stringify({ mode }),
  });
}

export function stopSession(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/stop`, {
    method: "POST",
  });
}

/** Opens the human's terminal in the session's folder: its worktree, else the project. */
export function openTerminal(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/terminal`, {
    method: "POST",
  });
}

/** Opens the human's file manager in the session's project folder (their repository). */
export function openFileManager(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/files`, {
    method: "POST",
  });
}

export function compactSession(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/compact`, {
    method: "POST",
  });
}

export interface DeleteSessionsResult {
  deleted: string[];
  failed: { id: string; error: string }[];
}

/** Deletes top-level sessions, their child sessions, logs and worktrees. */
export function deleteSessions(ids: string[]): Promise<DeleteSessionsResult> {
  return request<DeleteSessionsResult>("/api/sessions/delete", {
    method: "POST",
    body: JSON.stringify({ ids }),
  });
}

export interface EventPageEntry {
  seq: number;
  event: { type: string; data: Record<string, unknown> };
}

/**
 * At least the last `limit` events before `before` (the log's end when 0),
 * ascending, from the start of a turn. The thread loads its pages this way;
 * the live tail comes from the SSE stream.
 */
export function getEventPage(
  id: string,
  before: number,
  limit: number,
): Promise<EventPageEntry[]> {
  const params = new URLSearchParams({ limit: String(limit) });
  if (before > 0) params.set("before", String(before));
  return request<EventPageEntry[]>(
    `/api/sessions/${encodeURIComponent(id)}/events?${params}`,
  );
}

/** What sits under one answer, written by the server from the usage ledger. */
export interface CallUsage {
  messageId: string;
  /** The model that answered this call. */
  model: string;
  line: string;
  detail: string;
  /** On the last call of a turn only. */
  turnRecap: string | null;
}

export function getUsageCalls(id: string): Promise<CallUsage[]> {
  return request<CallUsage[]>(
    `/api/sessions/${encodeURIComponent(id)}/usage/calls`,
  );
}
