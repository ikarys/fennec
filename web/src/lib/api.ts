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
}

export interface SlashCommand {
  name: string;
  description: string;
}

/** openfox `ReasoningEffort`: how much a model may think. */
export type ReasoningEffort = 'none' | 'minimal' | 'low' | 'medium' | 'high' | 'xhigh' | 'max';

export interface ModelPrefs {
  maxTokens?: number;
  contextWindow?: number;
  compactionThreshold?: number;
  reasoningEffort?: ReasoningEffort | null;
}

export interface DisplayPrefs {
  reasoningOpen: boolean;
  toolResultsOpen: boolean;
  /** The UI's base font size in pixels (server default 14, range 13..=16). */
  fontSize: number;
}

export interface Preferences {
  model: string | null;
  models: Record<string, ModelPrefs>;
  display: DisplayPrefs;
  /** openfox `globalInstructions`: added to the system prompt of new sessions. */
  globalInstructions?: string | null;
}

export interface SettingsModel {
  id: string;
  contextWindow: number | null;
}

export interface SettingsDefaults {
  model: string;
  maxTokens: number;
  contextWindow: number;
  compactAt: number;
  reasoningEffort: ReasoningEffort | null;
}

export interface SettingsResponse {
  preferences: Preferences;
  models: SettingsModel[];
  defaults: SettingsDefaults;
}

export interface SessionDetail {
  id: string;
  title: string;
  state: SessionState;
  project: {
    id: string;
    name: string;
    workdir: string;
  };
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    headers: {
      'Content-Type': 'application/json'
    },
    ...init
  });

  if (!response.ok) {
    let message = response.statusText;
    try {
      const body = (await response.json()) as { error?: string };
      if (body.error) {
        message = body.error;
      }
    } catch {
      // Keep the status text when the error body is not JSON.
    }
    throw new Error(message);
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

export function getSession(id: string): Promise<SessionDetail> {
  return request<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`);
}

export function createSession(projectId: string, prompt: string): Promise<{ id: string }> {
  return request<{ id: string }>('/api/sessions', {
    method: 'POST',
    body: JSON.stringify({ projectId, prompt })
  });
}

export function sendMessage(id: string, content: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/messages`, {
    method: 'POST',
    body: JSON.stringify({ content })
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
