import type {
  Origin,
  WorkflowDef,
  WorkflowIssue,
  WorkflowSummary,
} from "./workflow";
import type { DecisionView } from "./fold";
import type { IdeaBoard, IdeaView } from "./ideas";

export type SessionState =
  | "running"
  | "complete"
  | "stopped"
  | "error"
  | "truncated"
  | "idle"
  | "waiting_for_user";

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

export interface Project {
  id: string;
  name: string;
  workdir: string;
  branch: string | null;
  remote: string | null;
  sessions: SessionSummary[];
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
export type ReasoningEffort =
  "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max";

export interface ModelPrefs {
  maxTokens?: number;
  contextWindow?: number;
  compactionThreshold?: number;
  reasoningEffort?: ReasoningEffort | null;
  price?: { inputPerM: number; cachedPerM: number; outputPerM: number } | null;
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
  webFetch?: boolean;
}

/** fennec's bwrap world: how commands run. */
export interface SandboxPrefs {
  enabled: boolean;
  network: boolean;
}

/** The ideas queue's caps in night mode, per idea; 0 means no cap. */
export interface AutonomyPrefs {
  maxMinutesPerIdea: number;
  maxTokensPerIdea: number;
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

export interface ModelChoice {
  provider: string;
  model: string;
  providerName: string;
}

export interface ModelChoices {
  default: ModelChoice;
  favorites: ModelChoice[];
}

export interface SessionModel {
  provider: string;
  model: string;
  providerName: string;
  contextWindow: number;
  compactAt: number;
  source: "session" | "parent" | "default";
}

export interface Preferences {
  selected: ModelRef | null;
  providers: Provider[];
  display: DisplayPrefs;
  /** Models the composer offers as favourites. */
  favorites?: ModelRef[];
  /** openfox `globalInstructions`: added to the system prompt of new sessions. */
  globalInstructions?: string | null;
  /** openfox search.*: the web_search tool's SearXNG instance. */
  search?: SearchPrefs;
  /** fennec's bwrap world: how commands run. */
  sandbox?: SandboxPrefs;
  autonomy?: AutonomyPrefs;
  /** Skill ids every new session gets in its system prompt. */
  alwaysOnSkills?: string[];
  /** Opt-in embedded workflows the human enabled. */
  enabledWorkflows?: string[];
}

/** Where a resolved parameter value comes from. */
export type ParamSource = "perModel" | "listed" | "default";

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
  /** Added by hand: stored with no listing (the endpoint has no /models route). */
  manual?: boolean;
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

export interface SkillSummary {
  id: string;
  description: string;
}

export interface SettingsResponse {
  preferences: Preferences;
  listings: ProviderListing[];
  defaults: SettingsDefaults;
  /** The skills a session can load outside any project: defaults and ~/.agents/skills. */
  skills: SkillSummary[];
  optInWorkflows: WorkflowOptIn[];
  scanners: ScannerStatus[];
}

/** A security scanner the audit-security agent runs; `path` is null when fennec does not find it on PATH. */
export interface ScannerStatus {
  id: string;
  name: string;
  covers: string;
  install: string;
  url: string;
  path: string | null;
}

export interface WorkflowOptIn {
  id: string;
  name: string;
  description: string;
}

export interface SessionDetail {
  id: string;
  title: string;
  state: SessionState;
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
  } | null;
  project: {
    id: string;
    name: string;
    workdir: string;
  };
  /** The idea this session frames, or null. */
  idea: { id: string; state: "inbox" | "framing" | "ready" } | null;
  /** The first message the human typed, verbatim, or null. */
  request: string | null;
  /** The last answer read as an audit report, or null. */
  audit: Audit | null;
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

/** Turns the checked findings of a session's audit into backlog ideas. */
export function createAuditTasks(sessionId: string, findings: number[]): Promise<{ created: number }> {
  return request<{ created: number }>(`/api/sessions/${encodeURIComponent(sessionId)}/audit/tasks`, {
    method: 'POST',
    body: JSON.stringify({ findings }),
  });
}

export class WorkflowSaveError extends Error {
  issues: WorkflowIssue[];
  constructor(message: string, issues: WorkflowIssue[]) {
    super(message);
    this.name = "WorkflowSaveError";
    this.issues = issues;
  }
}

export type IntegrationConflict = { files: string[]; command: string };

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    headers: {
      "Content-Type": "application/json",
    },
    ...init,
  });

  if (!response.ok) {
    let message = response.statusText;
    let errors: WorkflowIssue[] | undefined;
    let conflict: IntegrationConflict | undefined;
    try {
      const body = (await response.json()) as {
        error?: string;
        errors?: WorkflowIssue[];
        conflict?: IntegrationConflict;
      };
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
  const query = path ? `?path=${encodeURIComponent(path)}` : "";
  return request<FolderListing>(`/api/folders${query}`);
}

/** Create a new project folder `name` inside `path` (an empty git repo). */
export function createFolder(path: string, name: string): Promise<{ path: string }> {
  return request<{ path: string }>("/api/folders/create", {
    method: "POST",
    body: JSON.stringify({ path, name }),
  });
}

export function getSettings(): Promise<SettingsResponse> {
  return request<SettingsResponse>("/api/settings");
}

export function saveSettings(
  preferences: Preferences,
): Promise<SettingsResponse> {
  return request<SettingsResponse>("/api/settings", {
    method: "POST",
    body: JSON.stringify(preferences),
  });
}

/** Re-list one provider's models now (its baseUrl/key may not be stored yet). */
export function listProvider(body: {
  provider: string;
  baseUrl: string;
  apiKey: ApiKey | null;
}): Promise<ProviderListing> {
  return request<ProviderListing>("/api/settings/providers/list", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

/** Where a probed limit comes from: the endpoint's own words, or the model family's catalog entry. */
export type LimitSource = "endpoint" | "catalog";

/** What a probe of one model learned (indicative); null where nothing said it. */
export interface ProbedModel {
  contextWindow: number | null;
  contextSource: LimitSource | null;
  maxTokens: number | null;
  /** The distinct efforts accepted; empty when neither the endpoint nor the catalog says. */
  reasoningEfforts: ReasoningEffort[];
  effortsSource: LimitSource | null;
  defaultReasoningEffort: ReasoningEffort | null;
}

/** Probes a model of a stored provider for its limits, with two small calls. */
export function probeModel(
  provider: string,
  model: string,
): Promise<ProbedModel> {
  return request<ProbedModel>("/api/settings/models/probe", {
    method: "POST",
    body: JSON.stringify({ provider, baseUrl: "", apiKey: null, model }),
  });
}

export function getProjects(): Promise<Project[]> {
  return request<Project[]>("/api/projects");
}

export function createProject(workdir: string): Promise<Project> {
  return request<Project>("/api/projects", {
    method: "POST",
    body: JSON.stringify({ workdir }),
  });
}

/** An idea on the cross-project Backlog page: the board's idea plus its project. */
export type BacklogIdea = Omit<IdeaView, "attachments" | "plan"> & {
  project: { id: string; name: string };
};

/** A project's queue on the Backlog page: running, or with ideas ready. */
export interface BacklogQueue {
  project: { id: string; name: string };
  running: boolean;
  autonomous: boolean;
  ready: number;
  next: { id: string; text: string } | null;
}

/** `GET /api/backlog`: what waits, runs, is queued and got done, across projects. */
export interface BacklogView {
  total: { yours: number; running: number; ready: number; drafts: number; done: number; projects: number };
  caps: IdeaBoard["queue"]["caps"];
  yours: BacklogIdea[];
  running: BacklogIdea[];
  queues: BacklogQueue[];
  done: BacklogIdea[];
}

export function getBacklog(): Promise<BacklogView> {
  return request<BacklogView>("/api/backlog");
}

export function listCommands(projectId: string): Promise<SlashCommand[]> {
  return request<SlashCommand[]>(
    `/api/projects/${encodeURIComponent(projectId)}/commands`,
  );
}

export function listAgents(projectId: string): Promise<Agent[]> {
  return request<Agent[]>(
    `/api/projects/${encodeURIComponent(projectId)}/agents`,
  );
}

export function getSession(id: string): Promise<SessionDetail> {
  return request<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`);
}

export function getModelChoices(): Promise<ModelChoices> {
  return request<ModelChoices>("/api/models/choices");
}

export function setSessionModel(
  id: string,
  ref: ModelRef,
): Promise<{ model: SessionModel }> {
  return request<{ model: SessionModel }>(
    `/api/sessions/${encodeURIComponent(id)}/model`,
    {
      method: "POST",
      body: JSON.stringify(ref),
    },
  );
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

export function getUsage(id: string): Promise<SessionUsage> {
  return request<SessionUsage>(`/api/sessions/${encodeURIComponent(id)}/usage`);
}

/** How a criterion's status was established, as reported by the integration preview. */
export type IntegrationProvenance =
  | { kind: "verified"; command: string; exitCode?: number; output: string }
  | { kind: "indicative"; agent: string; reason: string };

export type IntegrationCriterionStatus =
  | { type: "pending" }
  | { type: "passed" | "failed"; provenance: IntegrationProvenance };

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

export interface Suppression {
  path: string;
  line: string;
  marker: string;
}

export type Blocker =
  | { kind: "noCommits" }
  | { kind: "criteriaNotPassed"; criteria: string[] }
  | { kind: "noCriteria" }
  | { kind: "protectedTouched"; paths: string[] }
  | { kind: "architectureViolated"; violations: ArchitectureViolation[] }
  | { kind: "suppressionsAdded"; suppressions: Suppression[] }
  | {
      kind: "testsWeakened";
      testsRemoved: number;
      assertionsRemoved: number;
      paths: string[];
    }
  | { kind: "branchChanged"; from: string; to: string | null }
  | { kind: "localChanges"; paths: string[] };

export interface IntegrationPreview {
  branch: string;
  base: string;
  humanBranch: string | null;
  originBranch: string | null;
  commits: { sha: string; subject: string }[];
  /** `carried`: the human's uncommitted work in the file, which the agent built on, goes into the integration commit. */
  files: {
    path: string;
    protected: boolean;
    carried: boolean;
    added: number;
    removed: number;
  }[];
  /** Test files (relative to the project) the integration could protect. */
  testFiles: string[];
  criteria: IntegrationCriterion[];
  /** Recorded by the agent: indicative, never a blocker. */
  decisions: DecisionView[];
  blockers: Blocker[];
  /** The session's idea the human may finish here, or null. */
  idea: { projectId: string; id: string } | null;
  /** A starting commit message (feat: <session title>); the human edits it. */
  defaultMessage: string;
}

/** The worktree's integration preview: what a cherry-pick would bring and what needs acceptance. */
/** The human's repository now: checked-out branch, the run's origin branch, uncommitted entries. */
export interface RepoState {
  dir: string;
  branch: string | null;
  originBranch: string | null;
  branchChanged: boolean;
  dirty: number;
}

export function getRepoState(sessionId: string): Promise<RepoState> {
  return request<RepoState>(
    `/api/sessions/${encodeURIComponent(sessionId)}/repo`,
  );
}

export function getIntegration(sessionId: string): Promise<IntegrationPreview> {
  return request<IntegrationPreview>(
    `/api/sessions/${encodeURIComponent(sessionId)}/integration`,
  );
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

export function integrate(
  sessionId: string,
  acknowledged: string[],
  message: string,
  protectTests: boolean,
  removeWorktree: boolean,
): Promise<IntegrationResult> {
  return request<IntegrationResult>(
    `/api/sessions/${encodeURIComponent(sessionId)}/integrate`,
    {
      method: "POST",
      body: JSON.stringify({
        acknowledged,
        message,
        protectTests,
        removeWorktree,
      }),
    },
  );
}

/** Drops the session's worktree, branch and base ref; the human's decision. */
export function discardWorktree(
  sessionId: string,
): Promise<{ commits: string[] }> {
  return request<{ commits: string[] }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/discard`,
    {
      method: "POST",
      body: "{}",
    },
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

/** One server the human lists in the project's `.agents/servers.json`. */
export interface ServerDef {
  name: string;
  command: string;
}

/** A server with its running state in this session. */
export interface ServerView extends ServerDef {
  running: boolean;
}

export function getServers(id: string): Promise<ServerView[]> {
  return request<ServerView[]>(`/api/sessions/${encodeURIComponent(id)}/servers`);
}

export function saveServers(id: string, servers: ServerDef[]): Promise<ServerView[]> {
  return request<ServerView[]>(`/api/sessions/${encodeURIComponent(id)}/servers`, {
    method: "POST",
    body: JSON.stringify({ servers }),
  });
}

export function startServer(id: string, name: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/servers/start`, {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}

export function stopServer(id: string, name: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/servers/stop`, {
    method: "POST",
    body: JSON.stringify({ name }),
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

/**
 * The `workflow.*` events of the latest run started before `before`
 * (the log's end when 0), ascending, from its `workflow.started`.
 */
export function getRunEventsBefore(
  id: string,
  before: number,
): Promise<EventPageEntry[]> {
  const params = new URLSearchParams();
  if (before > 0) params.set("before", String(before));
  return request<EventPageEntry[]>(
    `/api/sessions/${encodeURIComponent(id)}/workflow/run?${params}`,
  );
}

/** An ADR file of the project's `docs/adr/` (verified: read from disk). */
export interface Adr {
  number: number;
  file: string;
  title: string;
  status: string | null;
  date: string | null;
  body: string;
}

/** A module of the project map (verified: read from the code's imports), placed by the server. */
export interface MapModule {
  id: string;
  /** The modules linked by dependencies it is drawn with; "" when it has none. */
  group: string;
  files: number;
  tests: number;
  entry: boolean;
  layer: number;
  row: number;
  /** The first paragraph of its head file's leading doc comment (its authors' words). */
  summary: string | null;
  sources: MapSourceNote[];
  /** The modules it imports, and those importing it, the most imports first. */
  uses: MapLink[];
  usedBy: MapLink[];
  /** Top-left corner of its box (260 × 46), placed by the server. */
  x: number;
  y: number;
}

export interface MapSourceNote {
  path: string;
  summary: string | null;
}

export interface MapLink {
  module: string;
  imports: number;
}

export interface MapPoint {
  x: number;
  y: number;
}

export interface MapDependency {
  from: string;
  to: string;
  imports: number;
  /** The arrow as a finished SVG path: right angles, rounded corners, hops at crossings. */
  path: string;
  /** Where the import count is shown. */
  label: MapPoint;
}

export interface MapGroup {
  name: string;
  modules: number;
}

export interface ModuleMap {
  modules: MapModule[];
  dependencies: MapDependency[];
  /** In display order: the biggest first, "" (modules without dependencies) last. */
  groups: MapGroup[];
}

/** The committed `docs/map.md` against the map of the code as it is. */
export type MapFileState = "missing" | "stale" | "current";

/** A project's documentation file, read from disk (verified). */
export interface DocFile {
  path: string;
  name: string;
  content: string;
}

export interface ProjectOverview {
  map: ModuleMap;
  mapFile: MapFileState;
}

export function getProjectOverview(
  projectId: string,
): Promise<ProjectOverview> {
  return request<ProjectOverview>(
    `/api/projects/${encodeURIComponent(projectId)}/overview`,
  );
}

/** The project's documentation: its markdown files and its ADRs (verified). */
export function getProjectDocs(
  projectId: string,
): Promise<{ files: DocFile[]; adrs: Adr[] }> {
  return request<{ files: DocFile[]; adrs: Adr[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/docs`,
  );
}

export function writeMapFile(
  projectId: string,
): Promise<{ mapFile: MapFileState }> {
  return request<{ mapFile: MapFileState }>(
    `/api/projects/${encodeURIComponent(projectId)}/map-file`,
    {
      method: "POST",
      body: "{}",
    },
  );
}

export function listWorkflows(projectId: string): Promise<WorkflowSummary[]> {
  return request<WorkflowSummary[]>(
    `/api/projects/${encodeURIComponent(projectId)}/workflows`,
  );
}

export function getWorkflow(
  projectId: string,
  wid: string,
): Promise<{ origin: Origin; workflow: WorkflowDef }> {
  return request<{ origin: Origin; workflow: WorkflowDef }>(
    `/api/projects/${encodeURIComponent(projectId)}/workflows/${encodeURIComponent(wid)}`,
  );
}

export async function saveWorkflow(
  projectId: string,
  wid: string,
  scope: "project" | "global",
  workflow: WorkflowDef,
): Promise<{ origin: Origin; workflow: WorkflowDef }> {
  try {
    return await request<{ origin: Origin; workflow: WorkflowDef }>(
      `/api/projects/${encodeURIComponent(projectId)}/workflows/${encodeURIComponent(wid)}`,
      {
        method: "PUT",
        body: JSON.stringify({ scope, workflow }),
      },
    );
  } catch (error) {
    if (error instanceof Error) {
      const issues = (error as { issues?: WorkflowIssue[] }).issues ?? [];
      throw new WorkflowSaveError(error.message, issues);
    }
    throw error;
  }
}

export function validateWorkflow(
  projectId: string,
  workflow: WorkflowDef,
): Promise<WorkflowIssue[]> {
  return request<{ errors: WorkflowIssue[] }>("/api/workflows/validate", {
    method: "POST",
    body: JSON.stringify({ projectId, workflow }),
  }).then((result) => result.errors);
}

export function startWorkflow(
  sessionId: string,
  workflowId: string,
  task: string,
  stepByStep: boolean,
  worktree: boolean,
  attachments: AttachmentInput[] = [],
  plan?: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow`,
    {
      method: "POST",
      body: JSON.stringify({
        workflowId,
        task,
        stepByStep,
        worktree,
        ...(attachments.length > 0 ? { attachments } : {}),
        ...(plan ? { plan } : {}),
      }),
    },
  );
}

export function answerWorkflow(
  sessionId: string,
  runId: string,
  choice: string,
  comment: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/answer`,
    {
      method: "POST",
      body: JSON.stringify({ runId, choice, comment }),
    },
  );
}

export function continueWorkflow(
  sessionId: string,
  runId: string,
  comment: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/continue`,
    {
      method: "POST",
      body: JSON.stringify({ runId, comment }),
    },
  );
}

export function resumeWorkflow(
  sessionId: string,
  runId: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/resume`,
    {
      method: "POST",
      body: JSON.stringify({ runId }),
    },
  );
}

export function stopWorkflow(sessionId: string, runId: string): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/stop`,
    {
      method: "POST",
      body: JSON.stringify({ runId }),
    },
  );
}

export function pauseWorkflow(sessionId: string, runId: string): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/pause`,
    {
      method: "POST",
      body: JSON.stringify({ runId }),
    },
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
  return request<WorkflowUsage>(
    `/api/sessions/${encodeURIComponent(id)}/usage/workflow`,
  );
}

export interface InsightCard {
  label: string;
  value: string;
  detail: string;
}
export interface InsightDay {
  day: string;
  label: string;
  cachedPercent: number;
  freshPercent: number;
  lostPercent: number;
  title: string;
}
export interface InsightRow {
  key: string;
  name: string;
  calls: string;
  input: string;
  cache: string;
  output: string;
  lost: string;
  cost: string;
  time: string;
}
export interface InsightView {
  cards: InsightCard[];
  days: InsightDay[];
  projects: InsightRow[];
  models: InsightRow[];
  consumers: ContextComposition | null;
  projectOptions: { id: string; name: string }[];
}
export type InsightPeriod = "7" | "30" | "90" | "all";

export function getInsight(
  period: InsightPeriod,
  project: string | null,
): Promise<InsightView> {
  const params = new URLSearchParams({ days: period });
  if (project !== null) {
    params.set("project", project);
  }
  return request<InsightView>(`/api/insight?${params}`);
}
