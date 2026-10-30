/**
 * Project ideas backlog.
 * Contract: docs/api.md.
 */
import { request } from './api';

export type Stage = 'yours' | 'running' | 'queue' | 'drafts' | 'done';

export type IdeaStatus =
  | 'inbox'
  | 'framing'
  | 'queued'
  | 'running'
  | 'question'
  | 'review'
  | 'halted'
  | 'failed'
  | 'integrated';

export type IdeaAction = 'frame' | 'resumeFraming' | 'start' | 'open' | 'answer' | 'review';

export type IdeaMenuAction = 'reframe' | 'startUnframed' | 'drop';

export interface IdeaView {
  id: string;
  text: string;
  status: IdeaStatus;
  stage: Stage;
  action: IdeaAction | null;
  menu: IdeaMenuAction[];
  sessionId: string | null;
  plan: string | null;
  halted: string | null;
  autonomous: boolean;
  position: number | null;
  createdAt: number;
  startedAt: number | null;
  durationMs: number | null;
  tokens: number | null;
  criteria: { passed: number; failed: number; pending: number } | null;
  decisions: { title: string; choice: string }[];
  commits: number | null;
  files: number | null;
}

export interface IdeaBoard {
  queue: {
    running: boolean;
    autonomous: boolean;
    ready: number;
    next: { id: string; text: string } | null;
    caps: { maxMinutes: number; maxTokens: number };
  };
  groups: { stage: Stage; ideas: IdeaView[] }[];
  nightReport:
    | null
    | {
        startedAt: number;
        finishedAt: number;
        reason: string;
        autonomous: boolean;
        summary: {
          started: number;
          running: number;
          review: number;
          question: number;
          halted: number;
          failed: number;
          integrated: number;
        };
        ideas: IdeaView[];
      };
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

/** A plan written outside the backlog joins it as a ready idea of its session. */
export async function addPlannedIdea(
  projectId: string,
  text: string,
  sessionId: string,
  plan: string,
): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas', {
    method: 'POST',
    body: JSON.stringify({ text, sessionId, plan }),
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

export async function startQueue(projectId: string, autonomous: boolean): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas/queue/start', {
    method: 'POST',
    body: JSON.stringify({ autonomous }),
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

export async function reorderIdeas(projectId: string, ids: string[]): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas/reorder', {
    method: 'POST',
    body: JSON.stringify({ ids }),
  });
}

export async function markReportRead(projectId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas/report/read', {
    method: 'POST',
    body: '{}',
  });
}
