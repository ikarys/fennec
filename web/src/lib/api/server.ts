import { request } from './request';

export interface ServerSettings {
  port: number;
  runningPort: number;
  fromFlag: boolean;
  path: string;
  remote: boolean;
  runningRemote: boolean;
  passwordSet: boolean;
  restartNeeded: boolean;
}

export function getServer(): Promise<ServerSettings> {
  return request<ServerSettings>('/api/server');
}

export function setServer(port: number, remote: boolean): Promise<ServerSettings> {
  return request<ServerSettings>('/api/server', {
    method: 'POST',
    body: JSON.stringify({ port, remote }),
  });
}

export function setServerPassword(password: string): Promise<ServerSettings> {
  return request<ServerSettings>('/api/server/password', {
    method: 'POST',
    body: JSON.stringify({ password }),
  });
}

export function restartServer(): Promise<{ restarting: boolean }> {
  return request<{ restarting: boolean }>('/api/server/restart', {
    method: 'POST',
    body: JSON.stringify({}),
  });
}
