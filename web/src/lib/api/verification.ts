// The verification figures the server folds for a session's latest run.
import { request } from "./request";

export type CriterionOutcome =
  | { kind: "verified"; command: string; exitCode: number | null; passed: boolean }
  | { kind: "judged"; passed: boolean }
  | { kind: "pending" };

export interface CheckedCriterion {
  id: string;
  description: string;
  outcome: CriterionOutcome;
}

export interface Verification {
  runId: string | null;
  criteria: CheckedCriterion[];
  testsProtected: number;
  waiversAsked: number;
  waiversGranted: number;
  outsidePlan: string[] | null;
  block: string | null;
}

export function getVerification(id: string): Promise<Verification> {
  return request<Verification>(
    `/api/sessions/${encodeURIComponent(id)}/verification`,
  );
}
