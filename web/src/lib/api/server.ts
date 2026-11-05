import { request } from './request';

export interface ServerSettings {
  port: number;
  runningPort: number;
  fromFlag: boolean;
  path: string;
}

export function getServer(): Promise<ServerSettings> {
  return request<ServerSettings>('/api/server');
}

export function setServerPort(port: number): Promise<ServerSettings> {
  return request<ServerSettings>('/api/server', {
    method: 'POST',
    body: JSON.stringify({ port }),
  });
}

export function restartServer(): Promise<{ restarting: boolean }> {
  return request<{ restarting: boolean }>('/api/server/restart', {
    method: 'POST',
    body: JSON.stringify({}),
  });
}
