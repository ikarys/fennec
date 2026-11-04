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
  scope: "session" | "global";
  content: PluginBlockContent;
}

/** One plugin as defined in the global `~/.agents/plugins.json`. */
export interface GlobalPlugin {
  name: string;
  command: string;
  args: string[];
  /** The environment the plugin's process sees: the human must see what runs. */
  env: Record<string, string>;
  blocks: { id: string; title: string; scope: "session" | "global" }[];
  /** The human's activation, stored in fennec's store. */
  enabled: boolean;
  network: boolean;
  /** False when the definition or code changed since the human activated it. */
  active: boolean;
}

/** `GET /api/plugins`: the global definitions, their state, the global blocks. */
export function getGlobalPlugins(): Promise<{
  plugins: GlobalPlugin[];
  blocks: ServedPluginBlock[];
}> {
  return request<{ plugins: GlobalPlugin[]; blocks: ServedPluginBlock[] }>(`/api/plugins`);
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

/** `GET /api/sessions/{id}/plugins`: the session-scope blocks, for the left bar. */
export function getSessionPluginBlocks(
  sessionId: string,
): Promise<{ blocks: ServedPluginBlock[] }> {
  return request<{ blocks: ServedPluginBlock[] }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/plugins`,
  );
}
