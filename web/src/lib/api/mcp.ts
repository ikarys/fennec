// A session's MCP servers and their tools, as the composer toggles them.
import { request } from "./request";

/** One MCP tool as the composer sees it, with its token cost. */
export interface McpToolView {
  name: string;
  enabled: boolean;
  tokens: number;
}

/** One MCP server as the composer sees it. */
export interface McpServerView {
  name: string;
  visible: boolean;
  disabled: boolean;
  active: boolean;
  tokens: number;
  toolCount: number;
  /** The enabled tools' tokens, active or not: what switching the server on adds. */
  enabledToolTokens: number;
  tools: McpToolView[];
}

/** The session's MCP state and token totals (`GET /api/sessions/{id}/mcp`). */
export interface McpSessionView {
  servers: McpServerView[];
  totalTokens: number;
}

export function getSessionMcp(id: string): Promise<McpSessionView> {
  return request<McpSessionView>(`/api/sessions/${encodeURIComponent(id)}/mcp`);
}

/** Record the session's disabled MCP servers (`POST /api/sessions/{id}/mcp`). */
export function setSessionMcp(id: string, disabled: string[]): Promise<McpSessionView> {
  return request<McpSessionView>(`/api/sessions/${encodeURIComponent(id)}/mcp`, {
    method: "POST",
    body: JSON.stringify({ disabled }),
  });
}

/** The MCP view a new session would get (`POST /api/mcp`). */
export function getDraftMcp(disabled: string[] | null): Promise<McpSessionView> {
  return request<McpSessionView>("/api/mcp", {
    method: "POST",
    body: JSON.stringify({ disabled }),
  });
}
