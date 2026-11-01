import { request } from './request';

export type RuleKind = 'command' | 'tool' | 'path';
export type RuleAction = 'block' | 'ask';

export interface RuleView {
  id: string;
  scope: 'global' | 'project';
  kind: RuleKind;
  pattern: string;
  action: RuleAction;
  note: string | null;
  understood: string;
}

export interface NewRule {
  kind: RuleKind;
  pattern: string;
  action: RuleAction;
  note?: string;
}

export function getGlobalRules(): Promise<{ rules: RuleView[] }> {
  return request<{ rules: RuleView[] }>('/api/rules');
}

export function addGlobalRule(rule: NewRule): Promise<{ rules: RuleView[] }> {
  return request<{ rules: RuleView[] }>('/api/rules', {
    method: 'POST',
    body: JSON.stringify(rule),
  });
}

export function deleteRule(id: string): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/rules/${encodeURIComponent(id)}/delete`, {
    method: 'POST',
    body: '{}',
  });
}

export function getProjectRules(projectId: string): Promise<{ rules: RuleView[]; global: RuleView[] }> {
  return request<{ rules: RuleView[]; global: RuleView[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/rules`,
  );
}

export function addProjectRule(projectId: string, rule: NewRule): Promise<{ rules: RuleView[]; global: RuleView[] }> {
  return request<{ rules: RuleView[]; global: RuleView[] }>(
    `/api/projects/${encodeURIComponent(projectId)}/rules`,
    {
      method: 'POST',
      body: JSON.stringify(rule),
    },
  );
}
