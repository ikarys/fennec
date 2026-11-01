import { request } from './request';

export interface WorktreeEntry {
  name: string;
  sessionId: string | null;
  sessionTitle: string | null;
  running: boolean;
  updatedAt: number | null;
  bytes: number;
  generatedBytes: number;
  orphan: boolean;
  /** The shared Cargo cache: emptied, never deleted. */
  cache: boolean;
  repoMissing: boolean;
  foreignOwned: boolean;
  sudoCommand: string | null;
}

export interface ProjectWorktrees {
  scannedAt: number;
  totalBytes: number;
  generatedBytes: number;
  entries: WorktreeEntry[];
}

export interface WorktreeSummary {
  totalBytes: number;
  generatedBytes: number;
  alert: boolean;
  largestProjectId: string | null;
}

export function getProjectWorktrees(projectId: string, fresh: boolean): Promise<ProjectWorktrees> {
  const query = fresh ? 'fresh=1' : '';
  return request<ProjectWorktrees>(
    `/api/projects/${encodeURIComponent(projectId)}/worktrees${query ? `?${query}` : ''}`,
  );
}

export function getWorktreeSummary(): Promise<WorktreeSummary> {
  return request<WorktreeSummary>('/api/worktrees');
}

export function cleanWorktree(name: string): Promise<{ freedBytes: number }> {
  return request<{ freedBytes: number }>('/api/worktrees/clean', {
    method: 'POST',
    body: JSON.stringify({ name }),
  });
}

export function removeOrphanWorktree(name: string): Promise<{}> {
  return request<{}>('/api/worktrees/remove-orphan', {
    method: 'POST',
    body: JSON.stringify({ name }),
  });
}
