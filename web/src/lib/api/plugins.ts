import { request } from "./request";

/** A tint a plugin asks for, mapped to the core's tone primitives. */
export type PluginTone = "none" | "info" | "success" | "warning" | "danger";

/** One typed element of a plugin block; the core renders these, never HTML. */
export type PluginBlockElement =
  | { kind: "text"; text: string }
  | { kind: "figure"; label: string; value: string; tone?: PluginTone }
  | { kind: "badge"; label: string; tone?: PluginTone };

/** A block's data as a plugin emits it. */
export interface PluginBlockContent {
  elements: PluginBlockElement[];
}

/** A block as served to the UI: the plugin's name is the provenance badge. */
export interface ServedPluginBlock {
  plugin: string;
  title: string;
  scope: "session" | "project";
  content: PluginBlockContent;
}

/** One plugin as defined in the project's protected `.agents/plugins.json`. */
export interface ProjectPlugin {
  name: string;
  command: string;
  args: string[];
  blocks: { id: string; title: string; scope: "session" | "project" }[];
  /** The human's activation, stored in fennec's store. */
  enabled: boolean;
  network: boolean;
  /** False when the definition or code changed since the human activated it. */
  active: boolean;
}

/** `GET /api/projects/{id}/plugins`: the definitions, their state, the project blocks. */
export function getProjectPlugins(
  projectId: string,
): Promise<{ plugins: ProjectPlugin[]; blocks: ServedPluginBlock[] }> {
  return request<{ plugins: ProjectPlugin[]; blocks: ServedPluginBlock[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/plugins`,
  );
}

/** `POST /api/projects/{id}/plugins`: the human's activation (enabled, network). */
export function setProjectPlugin(
  projectId: string,
  name: string,
  enabled: boolean,
  network: boolean,
): Promise<{ name: string; enabled: boolean; network: boolean; active: boolean }> {
  return request<{ name: string; enabled: boolean; network: boolean; active: boolean }>(
    `/api/projects/${encodeURIComponent(projectId)}/plugins`,
    {
      method: "POST",
      body: JSON.stringify({ name, enabled, network }),
    },
  );
}

/** `GET /api/sessions/{id}/plugins`: the session-scope blocks, for the left bar. */
export function getSessionPluginBlocks(
  sessionId: string,
): Promise<{ blocks: ServedPluginBlock[] }> {
  return request<{ blocks: ServedPluginBlock[] }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/plugins`,
  );
}
