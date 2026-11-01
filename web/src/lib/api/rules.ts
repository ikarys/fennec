import { request } from './request';

export type RuleKind = 'command' | 'tool' | 'path';
export type RuleAction = 'allow' | 'ask' | 'block';
export type RuleAccess = 'read' | 'write' | 'readWrite';
export type RuleScope = 'global' | 'project' | 'session';
export type RuleOrigin = 'human' | { session: string };

export interface RuleView {
  id: number;
  scope: RuleScope;
  kind: RuleKind;
  pattern: string;
  access: RuleAccess | null;
  action: RuleAction;
  note: string | null;
  origin: RuleOrigin;
  understood: string;
}

export interface RulesOverview {
  global: RuleView[];
  projects: {
    id: string;
    name: string | null;
    rules: RuleView[];
  }[];
  sessions: {
    id: string;
    title: string | null;
    projectId: string | null;
    rules: RuleView[];
  }[];
}

export interface NewRule {
  scope: 'global' | { project: string };
  kind: RuleKind;
  pattern: string;
  access?: RuleAccess;
  action: RuleAction;
  note?: string;
}

export function getRules(): Promise<RulesOverview> {
  return request<RulesOverview>('/api/rules');
}

export function addRule(rule: NewRule): Promise<{ rule: RuleView }> {
  return request<{ rule: RuleView }>('/api/rules', {
    method: 'POST',
    body: JSON.stringify(rule),
  });
}

export function deleteRule(id: number): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/rules/${encodeURIComponent(String(id))}/delete`, {
    method: 'POST',
    body: '{}',
  });
}

export function getProjectRules(projectId: string): Promise<{ rules: RuleView[]; global: RuleView[] }> {
  return request<{ rules: RuleView[]; global: RuleView[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/rules`,
  );
}
