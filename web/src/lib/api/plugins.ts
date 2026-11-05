import { request } from "./request";

/** A tint a plugin asks for, mapped to the core's tone primitives. */
export type PluginTone = "none" | "info" | "success" | "warning" | "danger";

/** The scope a block's definition declares: per session, per project, or server-wide. */
export type PluginScope = "session" | "project" | "global";

/** The surface a block is placed on within its scope. */
export type PluginPlacement =
  | "panel"
  | "own_panel"
  | "status_bar"
  | "dashboard"
  | "settings";

/** One typed element of a plugin block; the core renders these, never HTML. */
export type PluginBlockElement =
  | { kind: "text"; text: string }
  | { kind: "figure"; label: string; value: string; tone?: PluginTone }
  | { kind: "badge"; label: string; tone?: PluginTone }
  | { kind: "list"; items: { text: string; tone?: PluginTone }[] }
  | { kind: "progress"; label: string; value: number; max: number; tone?: PluginTone }
  | { kind: "link"; label: string; url: string }
  | { kind: "table"; columns: string[]; rows: string[][] };

/** A block's data as a plugin emits it. */
export interface PluginBlockContent {
  elements: PluginBlockElement[];
}

/** A block as served to the UI: the plugin's name is the provenance badge. */
export interface ServedPluginBlock {
  plugin: string;
  title: string;
  scope: PluginScope;
  placement: PluginPlacement;
  content: PluginBlockContent;
}

/** One plugin as defined in the global `~/.agents/plugins.json`. */
export interface GlobalPlugin {
  name: string;
  command: string;
  args: string[];
  /** The environment the plugin's process sees: the human must see what runs. */
  env: Record<string, string>;
  blocks: { id: string; title: string; scope: PluginScope }[];
  /** The human's activation, stored in fennec's store. */
  enabled: boolean;
  network: boolean;
  /** False when the definition or code changed since the human activated it. */
  active: boolean;
}

/** `GET /api/plugins`: the global definitions, their state, why some were left out (`problems`), the global blocks. */
export interface GlobalPlugins {
  plugins: GlobalPlugin[];
  problems: string[];
  blocks: ServedPluginBlock[];
}

export function getGlobalPlugins(): Promise<GlobalPlugins> {
  return request<GlobalPlugins>(`/api/plugins`);
}

/** `POST /api/plugins`: the human's activation (enabled, network). */
export function setGlobalPlugin(
  name: string,
  enabled: boolean,
  network: boolean,
): Promise<{ name: string; enabled: boolean; network: boolean; active: boolean }> {
  return request<{ name: string; enabled: boolean; network: boolean; active: boolean }>(
    `/api/plugins`,
    {
      method: "POST",
      body: JSON.stringify({ name, enabled, network }),
    },
  );
}

/** `GET /api/sessions/{id}/plugins`: the session-scope blocks, for the right-hand panel. */
export function getSessionPluginBlocks(
  sessionId: string,
): Promise<{ blocks: ServedPluginBlock[] }> {
  return request<{ blocks: ServedPluginBlock[] }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/plugins`,
  );
}

/** `GET /api/projects/{id}/plugins`: the project-scope blocks, for the dashboard. */
export function getProjectPluginBlocks(
  projectId: string,
): Promise<{ blocks: ServedPluginBlock[] }> {
  return request<{ blocks: ServedPluginBlock[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/plugins`,
  );
}

/** `GET /api/plugins/status-bar`: the blocks placed in the status bar, by surface. */
export function getStatusBarPluginBlocks(params: {
  sessionId?: string;
  projectId?: string;
}): Promise<{ blocks: ServedPluginBlock[] }> {
  const query = new URLSearchParams();
  if (params.sessionId) query.set("sessionId", params.sessionId);
  if (params.projectId) query.set("projectId", params.projectId);
  const suffix = query.toString() ? `?${query.toString()}` : "";
  return request<{ blocks: ServedPluginBlock[] }>(`/api/plugins/status-bar${suffix}`);
}
