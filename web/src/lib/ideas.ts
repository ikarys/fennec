/**
 * Project ideas backlog.
 * Contract: docs/api.md.
 */
import { request } from './api';

export type IdeaState = 'inbox' | 'framing' | 'ready' | 'started';

export type IdeaSessionState =
  | 'running'
  | 'waiting_for_user'
  | 'complete'
  | 'error'
  | 'stopped'
  | 'truncated'
  | 'idle';

export interface Idea {
  id: string;
  text: string;
  state: IdeaState;
  sessionId: string | null;
  plan: string | null;
  sessionState: IdeaSessionState | null;
  createdAt: number;
  updatedAt: number;
}

export interface IdeaBoard {
  ideas: Idea[];
  queueRunning: boolean;
}

type BoardRequestInit = {
  method?: 'POST';
  body?: string;
};

const boardRequest = (
  projectId: string,
  path: string,
  init?: BoardRequestInit,
): Promise<IdeaBoard> =>
  request<IdeaBoard>(`/api/projects/${encodeURIComponent(projectId)}${path}`, init);

export async function listIdeas(projectId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas');
}

export async function addIdea(projectId: string, text: string): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas', {
    method: 'POST',
    body: JSON.stringify({ text }),
  });
}

export async function frameIdea(
  projectId: string,
  ideaId: string,
  sessionId: string,
): Promise<IdeaBoard> {
  return boardRequest(projectId, `/ideas/${encodeURIComponent(ideaId)}/framing`, {
    method: 'POST',
    body: JSON.stringify({ sessionId }),
  });
}

export async function markIdeaReady(
  projectId: string,
  ideaId: string,
  plan: string,
): Promise<IdeaBoard> {
  return boardRequest(projectId, `/ideas/${encodeURIComponent(ideaId)}/ready`, {
    method: 'POST',
    body: JSON.stringify({ plan }),
  });
}

export async function dropIdea(projectId: string, ideaId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, `/ideas/${encodeURIComponent(ideaId)}/drop`, {
    method: 'POST',
    body: '{}',
  });
}

export async function startQueue(projectId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas/queue/start', {
    method: 'POST',
    body: '{}',
  });
}

export async function stopQueue(projectId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas/queue/stop', {
    method: 'POST',
    body: '{}',
  });
}

/** Builds this idea now, outside the queue: from its plan when ready, else the workflow plans first. */
export async function startIdea(projectId: string, ideaId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, `/ideas/${encodeURIComponent(ideaId)}/start`, {
    method: 'POST',
    body: '{}',
  });
}
