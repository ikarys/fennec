import { describe, it, expect } from "vitest";
import { runWaiting, portTone, type RunView } from "./runView";
import type { NodeKind, WorkflowDef, WorkflowEdge, WorkflowNode } from "./workflow";

const node = (id: string, kind: NodeKind, extra: Partial<WorkflowNode> = {}): WorkflowNode => ({
  id,
  kind,
  position: { x: 0, y: 0 },
  ...extra
});

const edge = (from: string, port: string, to: string): WorkflowEdge => ({
  from,
  port,
  to
});

const def = (nodes: WorkflowNode[] = [], edges: WorkflowEdge[] = []): WorkflowDef => ({
  id: "wf",
  name: "wf",
  nodes,
  edges
});

// The run view is served pre-calculated by the server; the tests build it as data.
const viewOf = (overrides: Partial<RunView> = {}): RunView => ({
  runId: "run-1",
  workflow: def(),
  task: "task",
  stepByStep: false,
  pauseRequested: false,
  runs: [],
  open: [],
  questions: {},
  rounds: [],
  upcoming: [],
  counts: { done: 0, open: 0, total: 0 },
  ...overrides
});

describe("runWaiting", () => {
  it("returns undefined when there is no run", () => {
    expect(runWaiting(undefined, false, undefined)).toBeUndefined();
  });

  it("returns undefined while the run is running", () => {
    expect(runWaiting(viewOf(), true, undefined)).toBeUndefined();
  });

  it("returns relaunch for a finished error run", () => {
    expect(runWaiting(viewOf({ finished: { outcome: "error" } }), false, undefined)).toEqual({ kind: "relaunch" });
  });

  it("returns undefined for a finished non error run", () => {
    expect(runWaiting(viewOf({ finished: { outcome: "done" } }), false, undefined)).toBeUndefined();
  });

  it("returns a pause waiting shape with the target label", () => {
    const view = viewOf({
      workflow: def([node("a", "agent", { name: "A" }), node("b", "agent", { name: "Review" })], [edge("a", "next", "b")]),
      paused: { from: "a", port: "next", to: "b" }
    });
    expect(runWaiting(view, false, undefined)).toEqual({ kind: "pause", to: "b", toLabel: "Review" });
  });

  it("returns a human waiting shape with filled question and choices", () => {
    const view = viewOf({
      workflow: def(
        [node("h", "human", { name: "Ask", question: "raw question", choices: ["Yes", "No"] })],
        [edge("start", "next", "h")]
      ),
      open: ["h"],
      questions: { h: "filled question" }
    });
    expect(runWaiting(view, false, undefined)).toEqual({
      kind: "human",
      nodeId: "h",
      question: "filled question",
      choices: ["Yes", "No"]
    });
  });

  it("falls back to the raw human question when no asked event exists", () => {
    const view = viewOf({
      workflow: def(
        [node("h", "human", { name: "Ask", question: "raw question", choices: ["Go"] })],
        [edge("start", "next", "h")]
      ),
      open: ["h"]
    });
    expect(runWaiting(view, false, undefined)).toEqual({
      kind: "human",
      nodeId: "h",
      question: "raw question",
      choices: ["Go"]
    });
  });

  it("returns undefined for an open agent node when lastDone is waiting_for_user", () => {
    const view = viewOf({
      workflow: def([node("a", "agent", { name: "A" })], [edge("start", "next", "a")]),
      open: ["a"]
    });
    expect(runWaiting(view, false, "waiting_for_user")).toBeUndefined();
  });

  it("returns interrupted for other open nodes", () => {
    const view = viewOf({
      workflow: def([node("a", "agent", { name: "A" })], [edge("start", "next", "a")]),
      open: ["a"]
    });
    expect(runWaiting(view, false, "complete")).toEqual({ kind: "interrupted" });
  });
});

describe("portTone", () => {
  it("classifies positive ports", () => {
    const ports = ["next", "done", "pass", "success", "clean", "true", "go", "ok", "accept", "minor"];
    for (const port of ports) {
      expect(portTone(port)).toBe("positive");
    }
  });

  it("classifies negative ports", () => {
    const ports = ["fail", "failure", "findings", "false", "exhausted", "blocked", "stop", "revise"];
    for (const port of ports) {
      expect(portTone(port)).toBe("negative");
    }
  });

  it("classifies unknown ports as other", () => {
    expect(portTone("custom")).toBe("other");
    expect(portTone("")).toBe("other");
  });

  it("normalizes port casing and whitespace", () => {
    expect(portTone(" DONE ")).toBe("positive");
    expect(portTone("FAIL")).toBe("negative");
    expect(portTone(" Custom ")).toBe("other");
  });
});
