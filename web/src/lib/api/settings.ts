import { request } from "./request";
import type { ShortcutBinding } from "../shortcuts";

/** `ReasoningEffort`: how much a model may think. */
export type ReasoningEffort =
  "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max";

export const REASONING_EFFORTS: ReasoningEffort[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

export interface ModelPrefs {
  maxTokens?: number;
  contextWindow?: number;
  compactionThreshold?: number;
  reasoningEffort?: ReasoningEffort | null;
  /** The efforts a probe found the model accepts. */
  reasoningEfforts?: ReasoningEffort[];
  price?: { inputPerM: number; cachedPerM: number; outputPerM: number } | null;
}

export type EditorId = 'vscode' | 'vscode-insiders' | 'cursor' | 'windsurf' | 'vscodium';

/** `AgentStyle`: how the agents write to the human. */
export type AgentStyle = 'concise' | 'normal' | 'explanatory' | 'technical';

export interface DisplayPrefs {
  reasoningOpen: boolean;
  toolResultsOpen: boolean;
  /** The UI's base font size in pixels (server default 14, range 13..=16). */
  fontSize: number;
  /** The UI's display language (server default "en"). */
  language?: 'en' | 'fr';
  /** The IDE the "open in editor" links open (server default "vscode"). */
  editor?: EditorId;
  /** Short sounds when something new needs attention (server default true). */
  sounds?: boolean;
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

/** What fennec writes in the integration commit. */
export interface IntegrationPrefs {
  /** One `Assisted-by` trailer per model that wrote the work (server default true). */
  modelTrailers: boolean;
}

/** The ideas queue's caps in night mode, per idea; 0 means no cap. */
export interface AutonomyPrefs {
  maxMinutesPerIdea: number;
  maxTokensPerIdea: number;
}

/** `delegation.enabled` / `delegation.maxPerTurn`: the `delegate` tool, on by default. */
export interface DelegationPrefs {
  enabled: boolean;
  maxPerTurn: number;
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
  /** The efforts the model accepts; empty when unknown. */
  acceptedEfforts?: ReasoningEffort[];
  /** The effort used when the session chooses none: the agent's, else the model's setting. */
  defaultEffort?: ReasoningEffort | null;
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
  acceptedEfforts: ReasoningEffort[];
  /** The effort the human chose for the session; null is auto. */
  effort: ReasoningEffort | null;
  /** The effort used while `effort` is null: the agent's, else the model's setting. */
  defaultEffort: ReasoningEffort | null;
}

export interface Preferences {
  selected: ModelRef | null;
  providers: Provider[];
  /** Whether a model's spend counts toward the projects' budgets, keyed `provider/model`. */
  billing?: Record<string, boolean>;
  display: DisplayPrefs;
  /** Models the composer offers as favourites. */
  favorites?: ModelRef[];
  /** `globalInstructions`: added to the system prompt of new sessions. */
  globalInstructions?: string | null;
  /** `agentStyle`: how the agents write to the human, for new sessions. */
  agentStyle?: AgentStyle;
  /** `search.*`: the web_search tool's SearXNG instance. */
  search?: SearchPrefs;
  /** fennec's bwrap world: how commands run. */
  sandbox?: SandboxPrefs;
  autonomy?: AutonomyPrefs;
  delegation?: DelegationPrefs;
  integration?: IntegrationPrefs;
  /** Skill ids every new session gets in its system prompt. */
  alwaysOnSkills?: string[];
  /** Opt-in embedded workflows the human enabled. */
  enabledWorkflows?: string[];
  /** MCP servers, configured on the Settings page. */
  mcp?: McpServer[];
  /** The human's keyboard shortcut overrides, action → combo; an empty combo disables. */
  shortcuts?: Record<string, string>;
}

/** One tool of an MCP server, as listed by `tools/list`. */
export interface McpTool {
  name: string;
  description: string | null;
  inputSchema: unknown;
}

/** The access class a server declares for a tool; provenance is declared, not verified. */
export type ToolAccess = 'read' | 'write' | 'destructive' | 'undeclared';

/** One cached tool's access and estimated token cost, as computed by the settings route. */
export interface McpToolSummary {
  name: string;
  access: ToolAccess;
  tokens: number;
}

/** One MCP server as the human configured it on the Settings page. */
export interface McpServer {
  name: string;
  transport: "stdio" | "http";
  command?: string | null;
  args: string[];
  env: Record<string, string>;
  url?: string | null;
  headers: Record<string, string>;
  /** Optional auth token: Bearer header (http) / `MCP_AUTH_TOKEN` env var (stdio). */
  authToken?: string | null;
  /** Globally off: the server's tools are not sent to the model. */
  disabled: boolean;
  /** Hidden from the composer dropdown; independent of `disabled`. */
  visible: boolean;
  /** Tool names the human turned off for this server. */
  disabledTools: string[];
  /** The tool list from the last "Connect"; the definitions come from here. */
  cachedTools: McpTool[];
  /** Per-call timeout in seconds; 0 keeps the client's default (60). */
  timeout: number;
}

/** What `POST /api/settings/mcp/connect` returns: the server with a fresh cache. */
export interface McpConnectResponse {
  server: McpServer;
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
  /** The efforts the model accepts (as probed); empty when never probed. */
  acceptedEfforts: ReasoningEffort[];
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

/** A security scanner the audit-security agent runs; `path` is null when fennec does not find it on PATH. */
export interface ScannerStatus {
  id: string;
  name: string;
  covers: string;
  install: string;
  url: string;
  path: string | null;
}

/** Whether bwrap runs on this machine, probed by the server on each settings load (verified). */
export interface SandboxHealth {
  status: 'ready' | 'missing' | 'blocked';
  detail: string | null;
}

export interface WorkflowOptIn {
  id: string;
  name: string;
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
  sandboxHealth: SandboxHealth;
  /** Per server, each cached tool's access and estimated token cost. */
  mcpTools: Record<string, McpToolSummary[]>;
  /** Every keyboard shortcut, the stored override applied, in the Settings page's order. */
  shortcuts: ShortcutBinding[];
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

/** Open one MCP server, list its tools, and cache the list in the preferences. */
export function connectMcp(name: string): Promise<McpConnectResponse> {
  return request<McpConnectResponse>("/api/settings/mcp/connect", {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}

/** Start an MCP server's OAuth authorization: the URL of its consent page. */
export function authorizeMcp(name: string): Promise<{ url: string }> {
  return request<{ url: string }>("/api/settings/mcp/authorize", {
    method: "POST",
    body: JSON.stringify({ name }),
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

/** What a probe of one model learned (indicative); null where nothing said it. */
export interface ProbedModel {
  contextWindow: number | null;
  maxTokens: number | null;
  /** The distinct efforts accepted; empty when the endpoint refused every probe. */
  reasoningEfforts: ReasoningEffort[];
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

export function setSessionEffort(
  id: string,
  effort: ReasoningEffort | null,
): Promise<{ model: SessionModel }> {
  return request<{ model: SessionModel }>(
    `/api/sessions/${encodeURIComponent(id)}/effort`,
    {
      method: "POST",
      body: JSON.stringify({ effort }),
    },
  );
}
