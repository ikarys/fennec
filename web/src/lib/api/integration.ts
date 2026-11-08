// The review gate: a session branch's integration preview, its diffs, the integration itself.
import { request } from "./request";
import type { CriterionView, DecisionView } from "./sessions";

export interface ArchitectureViolation {
  path: string;
  line: string;
  prefix: string;
  forbidden: string;
}

export interface Suppression {
  path: string;
  line: string;
  marker: string;
}

export interface SecurityFinding {
  scanner: string;
  rule: string;
  path: string;
  line: number | null;
  detail: string;
}

export type ScanStatus = { scanner: string } & (
  | { state: 'ran'; findings: number }
  | { state: 'notInstalled' }
  | { state: 'notApplicable' }
  | { state: 'failed'; reason: string }
  | { state: 'running'; seconds: number }
);

/** A blocker; `acknowledgeable` says whether the human may accept it (the server decides). */
export type Blocker = { acknowledgeable: boolean } & (
  | { kind: "securityScanRunning" }
  | { kind: "noCommits" }
  | { kind: "conflict"; files: string[] }
  | { kind: "criteriaNotPassed"; criteria: string[] }
  | { kind: "noCriteria" }
  | { kind: "protectedTouched"; paths: string[] }
  | { kind: "sensitiveTouched"; paths: string[] }
  | { kind: "outsidePlan"; paths: string[] }
  | { kind: "securityFindings"; findings: SecurityFinding[] }
  | { kind: "architectureViolated"; violations: ArchitectureViolation[] }
  | { kind: "suppressionsAdded"; suppressions: Suppression[] }
  | {
      kind: "testsWeakened";
      testsRemoved: number;
      assertionsRemoved: number;
      paths: string[];
    }
  | { kind: "branchChanged"; from: string; to: string | null }
  | { kind: "localChanges"; paths: string[] }
  | { kind: "understandingMissing" }
);

export type ReviewSignal =
  | 'protected'
  | 'sensitive'
  | 'outsidePlan'
  | 'bypass'
  | 'architecture'
  | 'conflictMarker'
  | 'testsWeakened'
  | 'hot'
  | 'untested'
  | 'large';

/** Whether a comprehension dialogue was opened on the branch's current head. */
export type Understanding =
  | { state: 'none' }
  | { state: 'started'; sessionId: string }
  | { state: 'held'; sessionId: string };

export interface IntegrationPreview {
  branch: string;
  base: string;
  humanBranch: string | null;
  originBranch: string | null;
  commits: { sha: string; subject: string }[];
  /** `carried`: the human's uncommitted work in the file, which the agent built on, goes into the integration commit. */
  files: {
    path: string;
    protected: boolean;
    carried: boolean;
    added: number;
    removed: number;
    /** Deterministic review signals, riskiest files first (server order). */
    signals: ReviewSignal[];
  }[];
  /** Which security scanners ran on the branch (verified), and why the others did not. */
  securityScans: ScanStatus[];
  /** Signals every file carries (server-computed): shown once above the list, hidden on the rows. */
  commonSignals: ReviewSignal[];
  /** Test files (relative to the project) the integration could protect. */
  testFiles: string[];
  criteria: CriterionView[];
  /** Recorded by the agent: indicative, never a blocker. */
  decisions: DecisionView[];
  blockers: Blocker[];
  /** Whether a comprehension dialogue was opened on the branch's current head. */
  understanding: Understanding;
  /** The session's idea the human may finish here, or null. */
  idea: { projectId: string; id: string } | null;
  /** A starting commit message (feat: <session title>); the human edits it. */
  defaultMessage: string;
}

/** The worktree's integration preview: what a cherry-pick would bring and what needs acceptance. */
/** The human's repository now: checked-out branch, the run's origin branch, uncommitted entries. */
export interface RepoState {
  dir: string;
  branch: string | null;
  originBranch: string | null;
  branchChanged: boolean;
  dirty: number;
}

export function getRepoState(sessionId: string): Promise<RepoState> {
  return request<RepoState>(
    `/api/sessions/${encodeURIComponent(sessionId)}/repo`,
  );
}

export function getIntegration(sessionId: string): Promise<IntegrationPreview> {
  return request<IntegrationPreview>(
    `/api/sessions/${encodeURIComponent(sessionId)}/integration`,
  );
}

export type DiffLineKind = 'context' | 'added' | 'removed';

export interface DiffLine {
  kind: DiffLineKind;
  /** The line's number on each side; null on the side it is absent from. */
  old: number | null;
  new: number | null;
  text: string;
}

/** One file the session's branch changed, as numbered hunks (verified: read from git). */
export interface FileDiff {
  path: string;
  binary: boolean;
  /** Past 5000 lines the server drops the rest. */
  truncated: boolean;
  hunks: { header: string; lines: DiffLine[] }[];
  /** Opens the worktree's file in VS Code. */
  editorUrl: string;
}

export function getFileDiff(sessionId: string, path: string): Promise<FileDiff> {
  return request<FileDiff>(
    `/api/sessions/${encodeURIComponent(sessionId)}/diff?path=${encodeURIComponent(path)}`,
  );
}

/** Records that `dialogueId` is the comprehension dialogue of the reviewed session. */
export function startUnderstanding(sessionId: string, dialogueId: string): Promise<{ understanding: Understanding }> {
  return request<{ understanding: Understanding }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/understanding`,
    {
      method: 'POST',
      body: JSON.stringify({ dialogueId }),
    },
  );
}

/** An English commit subject the model proposes for the integration (indicative), or null. */
export function suggestCommitMessage(sessionId: string): Promise<{ message: string | null }> {
  return request<{ message: string | null }>(`/api/sessions/${encodeURIComponent(sessionId)}/integration/message`, {
    method: 'POST',
    body: JSON.stringify({}),
  });
}

/** Creates one commit with the worktree's changes on the human's branch after the accepted blockers. */
export interface IntegrationResult {
  head: string;
  commits: string[];
  protectedTests: string[];
  /** Set when the integration succeeded but `.agents/protected` could not be written. */
  protectError: string | null;
  /** The agent's folder and branch were removed along with the integration. */
  worktreeRemoved: boolean;
  /** Set when the integration succeeded but the worktree could not be removed. */
  removeError: string | null;
}

export function integrate(
  sessionId: string,
  acknowledged: string[],
  message: string,
  protectTests: boolean,
  removeWorktree: boolean,
): Promise<IntegrationResult> {
  return request<IntegrationResult>(
    `/api/sessions/${encodeURIComponent(sessionId)}/integrate`,
    {
      method: "POST",
      body: JSON.stringify({
        acknowledged,
        message,
        protectTests,
        removeWorktree,
      }),
    },
  );
}

/** Drops the session's worktree, branch and base ref; the human's decision. */
export function discardWorktree(
  sessionId: string,
): Promise<{ commits: string[] }> {
  return request<{ commits: string[] }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/discard`,
    {
      method: "POST",
      body: "{}",
    },
  );
}

/** Brings the session's branch up to date with the human's branch and starts a turn to resolve conflicts. */
export function resolveConflict(
  sessionId: string,
): Promise<{ conflicts: string[]; started: boolean }> {
  return request<{ conflicts: string[]; started: boolean }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/resolve-conflict`,
    {
      method: "POST",
      body: "{}",
    },
  );
}
