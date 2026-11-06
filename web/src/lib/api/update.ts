import { request } from './request';

export interface UpdateCheck {
  current: string;
  latest: string;
  newer: boolean;
  releaseUrl: string;
}

export function getUpdate(refresh = false): Promise<UpdateCheck> {
  const path = refresh ? '/api/update?refresh=1' : '/api/update';
  return request<UpdateCheck>(path);
}

export function applyUpdate(): Promise<{ restarting: boolean; latest: string }> {
  return request<{ restarting: boolean; latest: string }>('/api/update', {
    method: 'POST',
    body: JSON.stringify({}),
  });
}

export interface VersionInfo {
  version: string;
  /** The local build's number (`<commits>.<sha>[-dirty]`), null for a release. */
  build: string | null;
  changelogUrl: string;
}

export function getVersion(): Promise<VersionInfo> {
  return request<VersionInfo>('/api/version');
}
