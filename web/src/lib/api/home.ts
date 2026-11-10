// The home page's dashboard data: running count, recent sessions and favorites.
import { request } from './request';
import type { SessionSummary } from './sessions';

export interface HomeSession extends SessionSummary {
  projectId: string;
  projectName: string;
  chat: boolean;
}

export interface HomeFavorite {
  id: string;
  name: string;
  workdir: string;
  sessions: number;
  running: number;
}

export interface HomeView {
  running: number;
  recent: HomeSession[];
  favorites: HomeFavorite[];
}

export function getHome(): Promise<HomeView> {
  return request<HomeView>('/api/home');
}
