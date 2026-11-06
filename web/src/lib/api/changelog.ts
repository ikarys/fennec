import { request } from './request';

export interface ChangelogSection {
  title: string;
  items: string[];
}

export interface ChangelogRelease {
  version: string;
  date: string | null;
  sections: ChangelogSection[];
}

export interface Changelog {
  current: string;
  releases: ChangelogRelease[];
  unseen: number;
}

export const CHANGELOG_SEEN_EVENT = 'fennec:changelog-seen';

export function getChangelog(): Promise<Changelog> {
  return request<Changelog>('/api/changelog');
}

export function markChangelogSeen(): Promise<{ unseen: number }> {
  return request<{ unseen: number }>('/api/changelog/seen', {
    method: 'POST',
    body: JSON.stringify({}),
  });
}
