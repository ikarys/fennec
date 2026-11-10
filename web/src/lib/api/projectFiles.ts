// The project's files as listed and read on disk or on a session branch; files are saved, and tracked changes committed.
import { request } from "./request";

export interface ProjectFileEntry {
  path: string;
  tracked: boolean;
}

export interface ProjectFileListing {
  files: ProjectFileEntry[];
  truncated: boolean;
  /** True when read from the session's branch; false once that branch is gone (integrated). */
  session: boolean;
}

export interface ProjectFileView {
  path: string;
  content: string | null;
  binary: boolean;
  tooLarge: boolean;
  blob: string | null;
  editable: boolean;
  session: boolean;
}

export interface ProjectChange {
  path: string;
  status: 'modified' | 'added' | 'deleted';
}

export function getProjectFiles(
  projectId: string,
  sessionId: string | null,
): Promise<ProjectFileListing> {
  const params = new URLSearchParams();
  if (sessionId !== null) {
    params.set("session", sessionId);
  }
  const query = params.toString();

  return request<ProjectFileListing>(
    `/api/projects/${encodeURIComponent(projectId)}/files${
      query ? `?${query}` : ""
    }`,
  );
}

export function getProjectFile(
  projectId: string,
  path: string,
  sessionId: string | null,
): Promise<ProjectFileView> {
  const params = new URLSearchParams({ path });
  if (sessionId !== null) {
    params.set("session", sessionId);
  }

  return request<ProjectFileView>(
    `/api/projects/${encodeURIComponent(projectId)}/file?${params.toString()}`,
  );
}

export function saveProjectFile(
  projectId: string,
  input: { path: string; content: string; base: string },
): Promise<{ blob: string }> {
  return request<{ blob: string }>(
    `/api/projects/${encodeURIComponent(projectId)}/file`,
    {
      method: "POST",
      body: JSON.stringify(input),
    },
  );
}

export function getProjectChanges(projectId: string): Promise<{ files: ProjectChange[] }> {
  return request<{ files: ProjectChange[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/changes`,
  );
}

export function commitProjectFiles(
  projectId: string,
  input: { paths: string[]; message: string },
): Promise<{ commit: string }> {
  return request<{ commit: string }>(
    `/api/projects/${encodeURIComponent(projectId)}/commit`,
    {
      method: "POST",
      body: JSON.stringify(input),
    },
  );
}
