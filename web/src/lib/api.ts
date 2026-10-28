import type { Origin, WorkflowDef, WorkflowIssue, WorkflowSummary } from './workflow';
import type { DecisionView } from './fold';

export type SessionState =
  | 'running'
  | 'complete'
  | 'stopped'
  | 'error'
  | 'truncated'
  | 'idle'
  | 'waiting_for_user';

export interface SessionSummary {
  id: string;
  title: string;
  state: SessionState;
  createdAt: number;
  updatedAt: number;
  parentId: string | null;
  /** A workflow ended done and nothing integrated or discarded its work since. */
  awaitingIntegration: boolean;
}

export interface Project {
  id: string;
  name: string;
  workdir: string;
  branch: string | null;
  sessions: SessionSummary[];
}

export interface Config {
  model: string;
  contextWindow: number;
  /** The fraction of the window at which a session compacts; 0 disables. */
  compactAt: number;
}

export interface SlashCommand {
  name: string;
  description: string;
}

/** An agent: a mode of the session (`plan`, `builder`, …), or a sub-agent that workflow nodes run in a fresh context. */
export interface Agent {
  id: string;
  name: string;
  description: string;
  color: string | null;
  subagent: boolean;
}

/** openfox `ReasoningEffort`: how much a model may think. */
export type ReasoningEffort = 'none' | 'minimal' | 'low' | 'medium' | 'high' | 'xhigh' | 'max';

export interface ModelPrefs {
  maxTokens?: number;
  contextWindow?: number;
  compactionThreshold?: number;
  reasoningEffort?: ReasoningEffort | null;
}

export interface PanelPage {
  name: string;
  blocks: string[];
}

export interface DisplayPrefs {
  reasoningOpen: boolean;
  toolResultsOpen: boolean;
  /** The UI's base font size in pixels (server default 14, range 13..=16). */
  fontSize: number;
  /** The context panel's pages; null means the UI's defaults (lib/panelBlocks). */
  panelPages: PanelPage[] | null;
}

export interface SearchPrefs {
  searxngUrl?: string | null;
  searxngApiKey?: string | null;
}

/** fennec's bwrap world: how commands run. */
export interface SandboxPrefs {
  enabled: boolean;
  network: boolean;
}

export type ApiKey = { env: string } | { sealed: string } | { plain: string };

export interface Provider {
  /** Stable, without '/'. The 'local' one is the environment's endpoint (FENNEC_LLM_URL). */
  id: string;
  name: string;
  /** Empty for 'local' means the environment's URL. */
  baseUrl: string;
  apiKey: ApiKey | null;
  /** Keyed by the model id the provider lists. */
  models: Record<string, ModelPrefs>;
}

export interface ModelRef {
  provider: string;
  model: string;
}

export interface Preferences {
  selected: ModelRef | null;
  providers: Provider[];
  display: DisplayPrefs;
  /** openfox `globalInstructions`: added to the system prompt of new sessions. */
  globalInstructions?: string | null;
  /** openfox search.*: the web_search tool's SearXNG instance. */
  search?: SearchPrefs;
  /** fennec's bwrap world: how commands run. */
  sandbox?: SandboxPrefs;
}

/** Where a resolved parameter value comes from. */
export type ParamSource = 'perModel' | 'listed' | 'default';

/** One resolved parameter: its value, source, and whether a call sends it. */
export interface ResolvedValue<T> {
  value: T;
  source: ParamSource;
  wire: boolean;
}

/** What a model's turn runs with, display-ready (the UI computes none). */
export interface ResolvedParams {
  maxTokens: ResolvedValue<number>;
  contextWindow: ResolvedValue<number>;
  compactAt: ResolvedValue<number>;
  reasoningEffort: {
    value: ReasoningEffort | null;
    source: ParamSource;
    wire: boolean;
  };
}

export interface SettingsModel {
  id: string;
  contextWindow: number | null;
  resolved: ResolvedParams;
}

/** What one provider lists now, with the URL it is reached at. */
export interface ProviderListing {
  provider: string;
  name: string;
  baseUrl: string;
  models: SettingsModel[];
  /** Why the listing failed; null when it answered. */
  error: string | null;
}

export interface SettingsDefaults {
  provider: string;
  model: string;
  maxTokens: number;
  contextWindow: number;
  compactAt: number;
  reasoningEffort: ReasoningEffort | null;
}

export interface SettingsResponse {
  preferences: Preferences;
  listings: ProviderListing[];
  defaults: SettingsDefaults;
}

export interface SessionDetail {
  id: string;
  title: string;
  state: SessionState;
  /** The agent id the session resolves to, or null when it has none. */
  mode: string | null;
  /** The parent session id, or null for a top-level session. */
  parentId: string | null;
  /** The git worktree the session's agent works in, or null when it runs in place. */
  worktree: { path: string; branch: string; base: string; /** Opens the copy in VS Code (through the WSL remote under WSL). */ editorUrl: string } | null;
  project: {
    id: string;
    name: string;
    workdir: string;
  };
}

export class WorkflowSaveError extends Error {
  issues: WorkflowIssue[];
  constructor(message: string, issues: WorkflowIssue[]) {
    super(message);
    this.name = 'WorkflowSaveError';
    this.issues = issues;
  }
}

export type IntegrationConflict = { files: string[]; command: string };

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    headers: {
      'Content-Type': 'application/json'
    },
    ...init
  });

  if (!response.ok) {
    let message = response.statusText;
    let errors: WorkflowIssue[] | undefined;
    let conflict: IntegrationConflict | undefined;
    try {
      const body = (await response.json()) as { error?: string; errors?: WorkflowIssue[]; conflict?: IntegrationConflict };
      if (body.error) {
        message = body.error;
      }
      if (Array.isArray(body.errors)) {
        errors = body.errors;
      }
      if (body.conflict) {
        conflict = body.conflict;
      }
    } catch {
      // Keep the status text when the error body is not JSON.
    }
    const error = new Error(message);
    if (errors) {
      (error as { issues?: WorkflowIssue[] }).issues = errors;
    }
    if (conflict) {
      (error as { conflict?: IntegrationConflict }).conflict = conflict;
    }
    throw error;
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const text = await response.text();
  if (!text) {
    return undefined as T;
  }

  return JSON.parse(text) as T;
}

/** `message.done` stats of one model call (openfox `MessageStats` subset). */
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

export interface Folder {
  name: string;
  path: string;
  hidden: boolean;
  /** A git repository (has a `.git`). */
  repo: boolean;
}

export interface FolderListing {
  path: string;
  parent: string | null;
  folders: Folder[];
}

/** The folders inside `path` (the home directory when omitted). */
export function listFolders(path?: string): Promise<FolderListing> {
  const query = path ? `?path=${encodeURIComponent(path)}` : '';
  return request<FolderListing>(`/api/folders${query}`);
}

export function getConfig(): Promise<Config> {
  return request<Config>('/api/config');
}

export function getSettings(): Promise<SettingsResponse> {
  return request<SettingsResponse>('/api/settings');
}

export function saveSettings(preferences: Preferences): Promise<SettingsResponse> {
  return request<SettingsResponse>('/api/settings', {
    method: 'POST',
    body: JSON.stringify(preferences)
  });
}

/** Re-list one provider's models now (its baseUrl/key may not be stored yet). */
export function listProvider(body: {
  provider: string;
  baseUrl: string;
  apiKey: ApiKey | null;
}): Promise<ProviderListing> {
  return request<ProviderListing>('/api/settings/providers/list', {
    method: 'POST',
    body: JSON.stringify(body)
  });
}

export function getProjects(): Promise<Project[]> {
  return request<Project[]>('/api/projects');
}

export function createProject(workdir: string): Promise<Project> {
  return request<Project>('/api/projects', {
    method: 'POST',
    body: JSON.stringify({ workdir })
  });
}

export function listCommands(projectId: string): Promise<SlashCommand[]> {
  return request<SlashCommand[]>(`/api/projects/${encodeURIComponent(projectId)}/commands`);
}

export function listAgents(projectId: string): Promise<Agent[]> {
  return request<Agent[]>(`/api/projects/${encodeURIComponent(projectId)}/agents`);
}

export function getSession(id: string): Promise<SessionDetail> {
  return request<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`);
}

/** A session's skills folded from its log: loaded so far, and still in the model's view. */
export interface SessionSkills {
  loaded: string[];
  active: string[];
}

export function getSkills(id: string): Promise<SessionSkills> {
  return request<SessionSkills>(`/api/sessions/${encodeURIComponent(id)}/skills`);
}

/** A session's model calls from the server's usage ledger: every figure is computed there. */
export interface SessionUsage {
  calls: number;
  promptTokens: number;
  cachedTokens: number;
  completionTokens: number;
  /** 0 to 100. */
  cachePercent: number;
  /** Tokens per second of the last call; null before any call. */
  lastGenerationSpeed: number | null;
}

/** The session header's context gauge, computed by the server. */
export interface ContextView {
  /** 0 to 100, the bar's width. */
  percent: number;
  tone: 'ok' | 'warn' | 'err';
  /** Where the compaction threshold sits, 0 to 100; null when compaction is off. */
  compactMark: number | null;
  line: string;
  /** Taken, left, total, threshold. */
  title: string;
  /** Null while nothing is measured. */
  composition: ContextComposition | null;
}

export type ContextPartKind = 'prompt' | 'conversation' | 'tools';

/** Indicative: the exact total spread over the parts by their share of the text. */
export interface ContextComposition {
  kinds: { kind: ContextPartKind; label: string; tokens: string; percent: number }[];
  /** `kind` null: the row that sums the smallest parts. */
  parts: { kind: ContextPartKind | null; label: string; tokens: string; percent: number }[];
  images: string | null;
}

export function getContext(id: string): Promise<ContextView> {
  return request<ContextView>(`/api/sessions/${encodeURIComponent(id)}/context`);
}

export function getUsage(id: string): Promise<SessionUsage> {
  return request<SessionUsage>(`/api/sessions/${encodeURIComponent(id)}/usage`);
}

/** How a criterion's status was established, as reported by the integration preview. */
export type IntegrationProvenance =
  | { kind: 'verified'; command: string; exitCode?: number; output: string }
  | { kind: 'indicative'; agent: string; reason: string };

export type IntegrationCriterionStatus =
  | { type: 'pending' }
  | { type: 'passed' | 'failed'; provenance: IntegrationProvenance };

export interface IntegrationCriterion {
  id: string;
  description: string;
  check?: string;
  status: IntegrationCriterionStatus;
}

export interface ArchitectureViolation {
  path: string;
  line: string;
  prefix: string;
  forbidden: string;
}

export type Blocker =
  | { kind: 'noCommits' }
  | { kind: 'criteriaNotPassed'; criteria: string[] }
  | { kind: 'noCriteria' }
  | { kind: 'protectedTouched'; paths: string[] }
  | { kind: 'architectureViolated'; violations: ArchitectureViolation[] }
  | { kind: 'branchChanged'; from: string; to: string | null }
  | { kind: 'localChanges'; paths: string[] };

export interface IntegrationPreview {
  branch: string;
  base: string;
  humanBranch: string | null;
  originBranch: string | null;
  commits: { sha: string; subject: string }[];
  files: { path: string; protected: boolean; added: number; removed: number }[];
  /** Test files (relative to the project) the integration could protect. */
  testFiles: string[];
  criteria: IntegrationCriterion[];
  /** Recorded by the agent: indicative, never a blocker. */
  decisions: DecisionView[];
  blockers: Blocker[];
  /** A starting commit message (feat: <session title>); the human edits it. */
  defaultMessage: string;
}

/** The worktree's integration preview: what a cherry-pick would bring and what needs acceptance. */
export function getIntegration(sessionId: string): Promise<IntegrationPreview> {
  return request<IntegrationPreview>(`/api/sessions/${encodeURIComponent(sessionId)}/integration`);
}

/** Creates one commit with the worktree's changes on the human's branch after the accepted blockers. */
export interface IntegrationResult {
  head: string;
  commits: string[];
  protectedTests: string[];
  /** Set when the integration succeeded but `.agents/protected` could not be written. */
  protectError: string | null;
  /** The agent's folder and branch were removed along with the integration. */
  worktreeRemoved: boolean;
  /** Set when the integration succeeded but the worktree could not be removed. */
  removeError: string | null;
}

export function integrate(sessionId: string, acknowledged: string[], message: string, protectTests: boolean, removeWorktree: boolean): Promise<IntegrationResult> {
  return request<IntegrationResult>(`/api/sessions/${encodeURIComponent(sessionId)}/integrate`, {
    method: 'POST',
    body: JSON.stringify({ acknowledged, message, protectTests, removeWorktree })
  });
}

/** Drops the session's worktree, branch and base ref; the human's decision. */
export function discardWorktree(sessionId: string): Promise<{ commits: string[] }> {
  return request<{ commits: string[] }>(`/api/sessions/${encodeURIComponent(sessionId)}/discard`, {
    method: 'POST',
    body: '{}'
  });
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
  workflow?: { workflowId: string; stepByStep: boolean; worktree: boolean }
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
  return request<{ id: string }>('/api/sessions', {
    method: 'POST',
    body: JSON.stringify(body)
  });
}

export function sendMessage(id: string, content: string, attachments?: AttachmentInput[]): Promise<void> {
  const body: { content: string; attachments?: AttachmentInput[] } = { content };
  if (attachments && attachments.length > 0) {
    body.attachments = attachments;
  }
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/messages`, {
    method: 'POST',
    body: JSON.stringify(body)
  });
}

export function replayMessage(id: string, messageId: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/replay`, {
    method: 'POST',
    body: JSON.stringify({ messageId })
  });
}

export function setSessionMode(id: string, mode: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/mode`, {
    method: 'POST',
    body: JSON.stringify({ mode })
  });
}

export function stopSession(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/stop`, {
    method: 'POST'
  });
}

export function compactSession(id: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/compact`, {
    method: 'POST'
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
export function getEventPage(id: string, before: number, limit: number): Promise<EventPageEntry[]> {
  const params = new URLSearchParams({ limit: String(limit) });
  if (before > 0) params.set('before', String(before));
  return request<EventPageEntry[]>(`/api/sessions/${encodeURIComponent(id)}/events?${params}`);
}

/**
 * The `workflow.*` events of the latest run started before `before`
 * (the log's end when 0), ascending, from its `workflow.started`.
 */
export function getRunEventsBefore(id: string, before: number): Promise<EventPageEntry[]> {
  const params = new URLSearchParams();
  if (before > 0) params.set('before', String(before));
  return request<EventPageEntry[]>(`/api/sessions/${encodeURIComponent(id)}/workflow/run?${params}`);
}

export function listWorkflows(projectId: string): Promise<WorkflowSummary[]> {
  return request<WorkflowSummary[]>(`/api/projects/${encodeURIComponent(projectId)}/workflows`);
}

export function getWorkflow(projectId: string, wid: string): Promise<{ origin: Origin; workflow: WorkflowDef }> {
  return request<{ origin: Origin; workflow: WorkflowDef }>(
    `/api/projects/${encodeURIComponent(projectId)}/workflows/${encodeURIComponent(wid)}`
  );
}

export async function saveWorkflow(
  projectId: string,
  wid: string,
  scope: 'project' | 'global',
  workflow: WorkflowDef
): Promise<{ origin: Origin; workflow: WorkflowDef }> {
  try {
    return await request<{ origin: Origin; workflow: WorkflowDef }>(
      `/api/projects/${encodeURIComponent(projectId)}/workflows/${encodeURIComponent(wid)}`,
      {
        method: 'PUT',
        body: JSON.stringify({ scope, workflow })
      }
    );
  } catch (error) {
    if (error instanceof Error) {
      const issues = (error as { issues?: WorkflowIssue[] }).issues ?? [];
      throw new WorkflowSaveError(error.message, issues);
    }
    throw error;
  }
}

export function validateWorkflow(projectId: string, workflow: WorkflowDef): Promise<WorkflowIssue[]> {
  return request<{ errors: WorkflowIssue[] }>('/api/workflows/validate', {
    method: 'POST',
    body: JSON.stringify({ projectId, workflow })
  }).then((result) => result.errors);
}

export function startWorkflow(
  sessionId: string,
  workflowId: string,
  task: string,
  stepByStep: boolean,
  worktree: boolean,
  attachments: AttachmentInput[] = [],
  plan?: string
): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(sessionId)}/workflow`, {
    method: 'POST',
    body: JSON.stringify({
      workflowId,
      task,
      stepByStep,
      worktree,
      ...(attachments.length > 0 ? { attachments } : {}),
      ...(plan ? { plan } : {})
    })
  });
}

export function answerWorkflow(sessionId: string, runId: string, choice: string, comment: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(sessionId)}/workflow/answer`, {
    method: 'POST',
    body: JSON.stringify({ runId, choice, comment })
  });
}

export function continueWorkflow(sessionId: string, runId: string, comment: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(sessionId)}/workflow/continue`, {
    method: 'POST',
    body: JSON.stringify({ runId, comment })
  });
}

export function resumeWorkflow(sessionId: string, runId: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(sessionId)}/workflow/resume`, {
    method: 'POST',
    body: JSON.stringify({ runId })
  });
}

export function stopWorkflow(sessionId: string, runId: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(sessionId)}/workflow/stop`, {
    method: 'POST',
    body: JSON.stringify({ runId })
  });
}

/** What sits under one answer, written by the server from the usage ledger. */
export interface CallUsage {
  messageId: string;
  line: string;
  detail: string;
  /** On the last call of a turn only. */
  turnRecap: string | null;
}

export function getUsageCalls(id: string): Promise<CallUsage[]> {
  return request<CallUsage[]>(`/api/sessions/${encodeURIComponent(id)}/usage/calls`);
}

/** A workflow node run's token line, written by the server from the usage ledger. */
export interface NodeUsage {
  /** The thread item it belongs under. */
  itemId: string;
  line: string;
  detail: string;
}

export interface RunUsage {
  runId: string;
  calls: string;
  generated: string;
  read: string;
  cache: string;
}

export interface WorkflowUsage {
  nodes: NodeUsage[];
  runs: RunUsage[];
}

export function getUsageWorkflow(id: string): Promise<WorkflowUsage> {
  return request<WorkflowUsage>(`/api/sessions/${encodeURIComponent(id)}/usage/workflow`);
}

export interface InsightCard { label: string; value: string; detail: string }
export interface InsightDay { day: string; label: string; cachedPercent: number; freshPercent: number; lostPercent: number; title: string }
export interface InsightRow { key: string; name: string; calls: string; input: string; cache: string; output: string; lost: string; time: string }
export interface InsightView { scope: string; cards: InsightCard[]; days: InsightDay[]; projects: InsightRow[]; models: InsightRow[]; consumers: ContextComposition | null; projectOptions: { id: string; name: string }[] }
export type InsightPeriod = '7' | '30' | '90' | 'all';

export function getInsight(period: InsightPeriod, project: string | null): Promise<InsightView> {
  const params = new URLSearchParams({ days: period });
  if (project !== null) {
    params.set('project', project);
  }
  return request<InsightView>(`/api/insight?${params}`);
}
