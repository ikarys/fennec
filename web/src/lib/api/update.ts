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
