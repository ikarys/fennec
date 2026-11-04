import { request } from './request';

export type CanvasFormat = 'html' | 'markdown' | 'svg' | 'png' | 'jpeg' | 'gif' | 'webp';
export type CanvasPinLanguage = 'html' | 'markdown' | 'md' | 'svg';

export type CanvasSource = { kind: 'file'; path: string } | { kind: 'message' };

export interface CanvasArtifact {
  id: number;
  projectId: string;
  sessionId: string;
  title: string;
  format: CanvasFormat;
  source: CanvasSource;
  createdAt: number;
  updatedAt: number;
  latestVersion: number;
}

export interface CanvasVersion {
  number: number;
  size: number;
  createdAt: number;
}

export type CanvasAnchor =
  | { kind: 'whole' }
  | { kind: 'quote'; text: string }
  | { kind: 'region'; x: number; y: number; width: number; height: number };

export interface CanvasAnnotation {
  id: number;
  artifactId: number;
  version: number;
  anchor: CanvasAnchor;
  note: string;
  createdAt: number;
}

export interface CanvasDetail {
  artifact: CanvasArtifact;
  versions: CanvasVersion[];
  annotations: CanvasAnnotation[];
}

export function listSessionCanvas(sessionId: string): Promise<CanvasArtifact[]> {
  return request<{ artifacts: CanvasArtifact[] }>(`/api/sessions/${encodeURIComponent(sessionId)}/canvas`).then(
    (body) => body.artifacts,
  );
}

export function listProjectCanvas(projectId: string): Promise<CanvasArtifact[]> {
  return request<{ artifacts: CanvasArtifact[] }>(`/api/projects/${encodeURIComponent(projectId)}/canvas`).then(
    (body) => body.artifacts,
  );
}

export function pinCanvasFile(sessionId: string, path: string): Promise<CanvasArtifact> {
  return request<{ artifact: CanvasArtifact }>(`/api/sessions/${encodeURIComponent(sessionId)}/canvas`, {
    method: 'POST',
    body: JSON.stringify({ path }),
  }).then((body) => body.artifact);
}

export function pinCanvasBlock(sessionId: string, language: CanvasPinLanguage, content: string): Promise<CanvasArtifact> {
  return request<{ artifact: CanvasArtifact }>(`/api/sessions/${encodeURIComponent(sessionId)}/canvas`, {
    method: 'POST',
    body: JSON.stringify({ language, content }),
  }).then((body) => body.artifact);
}

export function getCanvas(id: number): Promise<CanvasDetail> {
  return request<CanvasDetail>(`/api/canvas/${encodeURIComponent(String(id))}`);
}

export function deleteCanvas(id: number): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/canvas/${encodeURIComponent(String(id))}/delete`, {
    method: 'POST',
    body: '{}',
  });
}

export function annotateCanvas(
  id: number,
  version: number,
  anchor: CanvasAnchor,
  note: string,
): Promise<{ annotation: CanvasAnnotation; message: string }> {
  return request<{ annotation: CanvasAnnotation; message: string }>(
    `/api/canvas/${encodeURIComponent(String(id))}/annotations`,
    {
      method: 'POST',
      body: JSON.stringify({ version, anchor, note }),
    },
  );
}

export function canvasVersionUrl(id: number, n: number): string {
  return `/api/canvas/${encodeURIComponent(String(id))}/versions/${encodeURIComponent(String(n))}`;
}
