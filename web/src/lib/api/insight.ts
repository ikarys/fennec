import { request } from "./request";
import type { ContextComposition } from "./sessions";

export interface InsightCard {
  label: string;
  value: string;
  detail: string;
}
export interface InsightDay {
  day: string;
  label: string;
  cachedPercent: number;
  freshPercent: number;
  lostPercent: number;
  title: string;
}
export interface InsightRow {
  key: string;
  name: string;
  calls: string;
  input: string;
  cache: string;
  output: string;
  lost: string;
  cost: string;
  time: string;
}
export interface InsightView {
  cards: InsightCard[];
  days: InsightDay[];
  projects: InsightRow[];
  models: InsightRow[];
  consumers: ContextComposition | null;
  projectOptions: { id: string; name: string }[];
}
export type InsightPeriod = "7" | "30" | "90" | "all";

export function getInsight(
  period: InsightPeriod,
  project: string | null,
): Promise<InsightView> {
  const params = new URLSearchParams({ days: period });
  if (project !== null) {
    params.set("project", project);
  }
  return request<InsightView>(`/api/insight?${params}`);
}
