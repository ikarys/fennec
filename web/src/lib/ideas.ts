/**
 * Project ideas backlog.
 * Contract: docs/api.md.
 */
import { request } from './api';

export type IdeaState = 'inbox' | 'framing' | 'ready';

export interface Idea {
  id: string;
  text: string;
  state: IdeaState;
  sessionId: string | null;
  plan: string | null;
  createdAt: number;
  updatedAt: number;
}

export async function listIdeas(projectId: string): Promise<Idea[]> {
  const data = await request<{ ideas: Idea[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/ideas`,
  );
  return data.ideas;
}

export async function addIdea(projectId: string, text: string): Promise<Idea[]> {
  const data = await request<{ ideas: Idea[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/ideas`,
    {
      method: 'POST',
      body: JSON.stringify({ text }),
    },
  );
  return data.ideas;
}

export async function frameIdea(
  projectId: string,
  ideaId: string,
  sessionId: string,
): Promise<Idea[]> {
  const data = await request<{ ideas: Idea[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/ideas/${encodeURIComponent(ideaId)}/framing`,
    {
      method: 'POST',
      body: JSON.stringify({ sessionId }),
    },
  );
  return data.ideas;
}

export async function markIdeaReady(
  projectId: string,
  ideaId: string,
  plan: string,
): Promise<Idea[]> {
  const data = await request<{ ideas: Idea[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/ideas/${encodeURIComponent(ideaId)}/ready`,
    {
      method: 'POST',
      body: JSON.stringify({ plan }),
    },
  );
  return data.ideas;
}

export async function dropIdea(projectId: string, ideaId: string): Promise<Idea[]> {
  const data = await request<{ ideas: Idea[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/ideas/${encodeURIComponent(ideaId)}/drop`,
    {
      method: 'POST',
      body: '{}',
    },
  );
  return data.ideas;
}
