import { request } from './request';
import type { AgentPhase } from './projects';
import type { ReasoningEffort } from './settings';

/** Where an agent's `*.agent.md` came from; only `user` is the human's to edit here. */
export type AgentOrigin = 'default' | 'user' | 'project';

/** An agent as the Settings page lists it: its definition plus its provenance. */
export interface AgentView {
  id: string;
  name: string;
  description: string;
  color: string | null;
  subagent: boolean;
  phase: AgentPhase | null;
  origin: AgentOrigin;
  path: string | null;
  editable: boolean;
  readOnly: boolean;
  allowedTools: string[];
}

/** An agent as the form edits it; the server renders the `*.agent.md`. */
export interface AgentDraft {
  id: string;
  name: string;
  description: string;
  phase: AgentPhase | null;
  color: string | null;
  subagent: boolean;
  /** `commands: read-only`: this agent's commands may not write. */
  readOnly: boolean;
  reasoningEffort: ReasoningEffort | null;
  allowedTools: string[];
  prompt: string;
}

/** A `*.agent.md` of the human's that discovery had to skip, and why. */
export interface BrokenAgent {
  id: string;
  error: string;
  field: string;
}

export interface AgentsOverview {
  agents: AgentView[];
  /** The tools an agent may name in `allowedTools`. */
  tools: string[];
  broken: BrokenAgent[];
}

function query(projectId?: string): string {
  return projectId ? `?projectId=${encodeURIComponent(projectId)}` : '';
}

export function getAgents(projectId?: string): Promise<AgentsOverview> {
  return request<AgentsOverview>(`/api/agents${query(projectId)}`);
}

/**
 * The human's `<id>.agent.md`: `fields` for the form, `content` the file
 * verbatim (`fields` null when the file does not parse).
 */
export function getAgentSource(id: string): Promise<{ id: string; content: string; fields: AgentDraft | null }> {
  return request<{ id: string; content: string; fields: AgentDraft | null }>(
    `/api/agents/${encodeURIComponent(id)}`,
  );
}

/** Writes the human's `<id>.agent.md` from the form's fields. */
export function saveAgent(draft: AgentDraft, projectId?: string): Promise<{ agent: AgentView }> {
  return request<{ agent: AgentView }>(`/api/agents${query(projectId)}`, {
    method: 'POST',
    body: JSON.stringify(draft),
  });
}

export function deleteAgent(id: string, projectId?: string): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/agents/${encodeURIComponent(id)}/delete${query(projectId)}`, {
    method: 'POST',
    body: '{}',
  });
}
