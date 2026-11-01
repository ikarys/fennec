/**
 * Project ideas backlog.
 * Contract: docs/api.md.
 */
import { request } from './api';
import type { FileAttachmentInput } from './attachments';

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
  | 'integrated'
  | 'completed';

export type IdeaAction = 'frame' | 'resumeFraming' | 'start' | 'open' | 'answer' | 'review';

export type IdeaMenuAction = 'restart' | 'reframe' | 'startUnframed' | 'complete' | 'drop';

/** A file or image pinned to an idea, kept as a data URL. */
export interface IdeaAttachmentView {
  filename: string;
  mimeType: string;
  size: number;
  data: string;
}

export interface IdeaView {
  id: string;
  text: string;
  /** A short title: the approved plan's, else the text's first sentence. */
  title: string;
  /** The text under the title, computed by the server. */
  description: string;
  attachments: IdeaAttachmentView[];
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
  updatedAt: number;
  startedAt: number | null;
  durationMs: number | null;
  tokens: number | null;
  criteria: { passed: number; failed: number; pending: number } | null;
  decisions: { title: string; choice: string }[];
  commits: number | null;
  files: number | null;
}

export interface RunSummary {
  started: number;
  running: number;
  review: number;
  question: number;
  halted: number;
  failed: number;
  integrated: number;
}

export interface IdeaBoard {
  queue: {
    running: boolean;
    autonomous: boolean;
    ready: number;
    next: { id: string; text: string; title: string } | null;
    caps: { maxMinutes: number; maxTokens: number };
  };
  groups: { stage: Stage; ideas: IdeaView[] }[];
  /** Every idea's status, counted by the server. */
  summary: RunSummary;
  nightReport:
    | null
    | {
        startedAt: number;
        finishedAt: number;
        reason: string;
        autonomous: boolean;
        summary: RunSummary;
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

export async function addIdea(
  projectId: string,
  text: string,
  attachments: FileAttachmentInput[] = [],
): Promise<IdeaBoard> {
  return boardRequest(projectId, '/ideas', {
    method: 'POST',
    body: JSON.stringify({ text, attachments }),
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

/** What a framing idea puts in the composer: text files inlined, binaries
 *  named, images attached. The plan is written against all of it. */
export function framingPrefill(idea: IdeaView): {
  text: string;
  attachments: { filename: string; data: string }[];
} {
  let text = idea.text;
  const attachments: { filename: string; data: string }[] = [];
  const named: string[] = [];
  for (const attachment of idea.attachments) {
    if (attachment.mimeType.startsWith('image/')) {
      attachments.push({ filename: attachment.filename, data: attachment.data });
    } else if (isTextMime(attachment.mimeType)) {
      const content = decodeDataUrl(attachment.data);
      // A file with its own code fences would break a triple-backtick wrapper.
      const fence = content.includes('```') ? '````' : '```';
      text += `\n\nAttached file \`${attachment.filename}\`:\n\n${fence}\n${content}\n${fence}`;
    } else {
      named.push(attachment.filename);
    }
  }
  if (named.length > 0) {
    text += `\n\nAttached files (available when the idea is built): ${named.join(', ')}`;
  }
  return { text, attachments };
}

function isTextMime(mime: string): boolean {
  return (
    mime.startsWith('text/') ||
    ['application/json', 'application/xml', 'application/yaml', 'application/x-yaml', 'application/csv'].includes(mime)
  );
}

function decodeDataUrl(dataUrl: string): string {
  const payload = dataUrl.split(';base64,').pop() ?? '';
  try {
    // atob alone returns one char per byte; multi-octet UTF-8 text needs a
    // TextDecoder to come back readable.
    const bytes = Uint8Array.from(atob(payload), (c) => c.charCodeAt(0));
    return new TextDecoder('utf-8').decode(bytes);
  } catch {
    return '';
  }
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

export async function completeIdea(projectId: string, ideaId: string): Promise<IdeaBoard> {
  return boardRequest(projectId, `/ideas/${encodeURIComponent(ideaId)}/complete`, {
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
