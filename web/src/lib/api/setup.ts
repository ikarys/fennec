import { request } from "./request";

export type SetupCheckId = "model" | "sandbox" | "git" | "project" | "scanners";
export type SetupStatus = "ok" | "todo" | "warn";
export interface SetupCheck {
  id: SetupCheckId;
  status: SetupStatus;
  detail: string | null;
}
export interface Setup {
  checks: SetupCheck[];
  complete: boolean;
}

/** Whether a model is named: a session needs one. No probe, so a click stays instant. */
export async function hasModel(): Promise<boolean> {
  const config = await request<{ model: string }>("/api/config");
  return config.model !== "";
}

/** The first-run checklist, probed by the server on each call. */
export function getSetup(): Promise<Setup> {
  return request<Setup>("/api/setup");
}
