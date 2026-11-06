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

/** The first-run checklist, probed by the server on each call. */
export function getSetup(): Promise<Setup> {
  return request<Setup>("/api/setup");
}
