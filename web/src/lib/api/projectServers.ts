// The project's own servers (`.agents/servers.json`), started from a session.
import { request } from "./request";

/** One server the human lists in the project's `.agents/servers.json`. */
export interface ServerDef {
  name: string;
  command: string;
}
/** A server's state in this session: `starting` until its port answers. */
/** A server's state in this session:  until its port answers. */
export type ServerState = 'stopped' | 'starting' | 'ready' | 'exited';

/** A server with its running state in this session. */
export interface ServerView extends ServerDef {
  suggested: boolean;
  state: ServerState;
  running: boolean;
  url: string | null;
  exitCode: number | null;
  output: string[];
}

export function getServers(id: string): Promise<ServerView[]> {
  return request<ServerView[]>(`/api/sessions/${encodeURIComponent(id)}/servers`);
}

export function saveServers(id: string, servers: ServerDef[]): Promise<ServerView[]> {
  return request<ServerView[]>(`/api/sessions/${encodeURIComponent(id)}/servers`, {
    method: "POST",
    body: JSON.stringify({ servers }),
  });
}

export function startServer(id: string, name: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/servers/start`, {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}

export function stopServer(id: string, name: string): Promise<void> {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}/servers/stop`, {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}
