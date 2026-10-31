// API types and client for project risk reports.
import { request } from './request';

export type RiskSignal = 'unreviewed' | 'untested' | 'churn';
export interface RiskZone { path: string; signals: RiskSignal[]; churn: number; lines: number; unreviewedPercent: number }
export interface RiskReport {
  files: number;
  unreviewedPercent: number;
  untested: number;
  churning: number;
  unreviewed: number;
  churnDays: number;
  zones: RiskZone[];
}

export function getProjectRisks(projectId: string): Promise<RiskReport> {
  return request<RiskReport>(`/api/projects/${encodeURIComponent(projectId)}/risks`);
}
