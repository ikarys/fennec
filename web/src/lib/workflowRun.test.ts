import { describe, it, expect } from "vitest";
import { createRunFold, runWaiting, upcomingSteps, runRounds, portTone, type NodeRun, type RunView } from "./workflowRun";
import type { NodeKind, WorkflowDef, WorkflowEdge, WorkflowNode } from "./workflow";

const ev = (type: string, data: Record<string, unknown> = {}) => ({ type, data });

const node = (id: string, kind: NodeKind, name?: string): WorkflowNode => ({
  id,
  kind,
  name,
  position: { x: 0, y: 0 }
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

const viewOf = (fold: ReturnType<typeof createRunFold>): RunView => {
  const view = fold.view();
  expect(view).toBeDefined();
  return view as RunView;
};

const started = (fold: ReturnType<typeof createRunFold>, workflow: WorkflowDef = def()): void => {
  fold.apply(
    ev("workflow.started", {
      runId: "run-1",
      workflow,
      task: "task",
      stepByStep: false
    })
  );
};

describe("createRunFold", () => {
  it("applies workflow events and ignores non workflow events", () => {
    const fold = createRunFold();
    expect(
      fold.apply(
        ev("workflow.started", {
          runId: "r",
          workflow: def(),
          task: "t",
          stepByStep: false
        })
      )
    ).toBe(true);
    expect(fold.apply(ev("message.start", { messageId: "m", role: "user" }))).toBe(false);
    expect(fold.apply(ev("chat.done", { messageId: "m", reason: "complete" }))).toBe(false);
  });

  it("returns true for workflow events before a run starts", () => {
    const fold = createRunFold();
    expect(fold.apply(ev("workflow.node.started", { runId: "r", nodeId: "a", pass: 1 }))).toBe(true);
    expect(fold.view()).toBeUndefined();
  });

  it("stores the workflow definition, task and step by step flag", () => {
    const fold = createRunFold();
    const workflow = def([node("a", "agent", "A")], [edge("start", "next", "a")]);
    fold.apply(
      ev("workflow.started", {
        runId: "run-1",
        workflow,
        task: "build",
        stepByStep: true
      })
    );

    const view = viewOf(fold);
    expect(view.runId).toBe("run-1");
    expect(view.workflow).toEqual(workflow);
    expect(view.task).toBe("build");
    expect(view.stepByStep).toBe(true);
    expect(view.pauseRequested).toBe(false);
    expect(view.runs).toEqual([]);
    expect(view.open).toEqual([]);
    expect(view.questions).toEqual({});
  });

  it("starts a node run with its label, kind and startedAt", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1, startedAt: 1000 }));

    const view = viewOf(fold);
    expect(view.runs).toHaveLength(1);
    expect(view.runs[0].nodeId).toBe("a");
    expect(view.runs[0].label).toBe("A");
    expect(view.runs[0].kind).toBe("agent");
    expect(view.runs[0].pass).toBe(1);
    expect(view.runs[0].state).toBe("running");
    expect(view.runs[0].startedAt).toBe(1000);
    expect(view.open).toEqual(["a"]);
  });

  it("finishes a node run with its port, duration and output", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(
      ev("workflow.node.finished", {
        runId: "run-1",
        nodeId: "a",
        port: "done",
        durationMs: 50,
        output: "ok"
      })
    );

    const view = viewOf(fold);
    expect(view.runs[0].state).toBe("done");
    expect(view.runs[0].port).toBe("done");
    expect(view.runs[0].durationMs).toBe(50);
    expect(view.runs[0].output).toBe("ok");
    expect(view.open).toEqual([]);
  });

  it("records a second pass of the same node as a second run", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(ev("workflow.node.finished", { runId: "run-1", nodeId: "a", port: "done" }));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 2 }));

    const view = viewOf(fold);
    expect(view.runs).toHaveLength(2);
    expect(view.runs[1].pass).toBe(2);
    expect(view.runs[1].state).toBe("running");
    expect(view.open).toEqual(["a"]);
  });

  it("tracks open nodes until they finish", () => {
    const fold = createRunFold();
    started(
      fold,
      def([node("a", "agent", "A"), node("b", "agent", "B")], [
        edge("start", "next", "a"),
        edge("a", "next", "b")
      ])
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "b", pass: 1 }));
    expect(viewOf(fold).open).toEqual(["a", "b"]);

    fold.apply(ev("workflow.node.finished", { runId: "run-1", nodeId: "a", port: "done" }));
    expect(viewOf(fold).open).toEqual(["b"]);

    fold.apply(ev("workflow.node.finished", { runId: "run-1", nodeId: "b", port: "done" }));
    expect(viewOf(fold).open).toEqual([]);
  });

  it("stores a pause request and then a paused edge", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A"), node("b", "agent", "B")], [edge("a", "next", "b")]));
    fold.apply(ev("workflow.pause_requested", { runId: "run-1" }));
    expect(viewOf(fold).pauseRequested).toBe(true);

    fold.apply(ev("workflow.paused", { runId: "run-1", from: "a", port: "next", to: "b" }));
    const view = viewOf(fold);
    expect(view.pauseRequested).toBe(false);
    expect(view.paused).toEqual({ from: "a", port: "next", to: "b" });
  });

  it("stores human node questions by node id", () => {
    const fold = createRunFold();
    started(fold, def([node("h", "human", "Ask")], [edge("start", "next", "h")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "h", pass: 1 }));
    fold.apply(ev("workflow.human.asked", { runId: "run-1", nodeId: "h", question: "Continue?" }));

    expect(viewOf(fold).questions).toEqual({ h: "Continue?" });
  });

  it("marks a finished run with its outcome and reason", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(
      ev("workflow.finished", {
        runId: "run-1",
        outcome: "blocked",
        reason: "criteria failed"
      })
    );

    const view = viewOf(fold);
    expect(view.finished).toEqual({ outcome: "blocked", reason: "criteria failed" });
    expect(view.open).toEqual([]);
    expect(view.pauseRequested).toBe(false);
  });

  it("ends a running node with the run outcome when the run finishes", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(ev("workflow.finished", { runId: "run-1", outcome: "stopped" }));

    const view = viewOf(fold);
    expect(view.runs[0].state).toBe("ended");
    expect(view.runs[0].port).toBe("stopped");
    expect(view.finished?.outcome).toBe("stopped");
  });

  it("supports done, blocked, stopped and error outcomes", () => {
    const outcomes = ["done", "blocked", "stopped", "error"] as const;
    for (const outcome of outcomes) {
      const fold = createRunFold();
      started(fold);
      fold.apply(ev("workflow.finished", { runId: "run-1", outcome }));
      expect(viewOf(fold).finished?.outcome).toBe(outcome);
    }
  });

  it("interrupts running nodes on resume and stores the comment", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(ev("workflow.paused", { runId: "run-1", from: "a", port: "next", to: "a" }));
    fold.apply(ev("workflow.resumed", { runId: "run-1", comment: "go" }));

    const view = viewOf(fold);
    expect(view.runs[0].state).toBe("interrupted");
    expect(view.paused).toBeUndefined();
    expect(view.open).toEqual([]);
    expect(view.lastComment).toBe("go");
  });

  it("clears a finished state when a finished run is resumed", () => {
    const fold = createRunFold();
    started(fold);
    fold.apply(ev("workflow.finished", { runId: "run-1", outcome: "error" }));
    fold.apply(ev("workflow.resumed", { runId: "run-1", comment: "retry" }));

    const view = viewOf(fold);
    expect(view.finished).toBeUndefined();
    expect(view.lastComment).toBe("retry");
  });

  it("replaces the first run when a second run starts", () => {
    const fold = createRunFold();
    started(fold);
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    fold.apply(
      ev("workflow.started", {
        runId: "run-2",
        workflow: def(),
        task: "second",
        stepByStep: true
      })
    );

    const view = viewOf(fold);
    expect(view.runId).toBe("run-2");
    expect(view.task).toBe("second");
    expect(view.stepByStep).toBe(true);
    expect(view.runs).toEqual([]);
    expect(view.open).toEqual([]);
    expect(view.questions).toEqual({});
  });

  it("returns a copied view so mutations do not leak into the fold", () => {
    const fold = createRunFold();
    started(fold);
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    const view = viewOf(fold);
    view.runs.push({
      nodeId: "b",
      label: "B",
      kind: "agent",
      pass: 1,
      state: "running"
    });

    expect(viewOf(fold).runs).toHaveLength(1);
  });
});

describe("runWaiting", () => {
  it("returns undefined when there is no run", () => {
    expect(runWaiting(undefined, false, undefined)).toBeUndefined();
  });

  it("returns undefined while the run is running", () => {
    const fold = createRunFold();
    started(fold);
    expect(runWaiting(viewOf(fold), true, undefined)).toBeUndefined();
  });

  it("returns relaunch for a finished error run", () => {
    const fold = createRunFold();
    started(fold);
    fold.apply(ev("workflow.finished", { runId: "run-1", outcome: "error" }));
    expect(runWaiting(viewOf(fold), false, undefined)).toEqual({ kind: "relaunch" });
  });

  it("returns undefined for a finished non error run", () => {
    const fold = createRunFold();
    started(fold);
    fold.apply(ev("workflow.finished", { runId: "run-1", outcome: "done" }));
    expect(runWaiting(viewOf(fold), false, undefined)).toBeUndefined();
  });

  it("returns a pause waiting shape with the target label", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A"), node("b", "agent", "Review")], [edge("a", "next", "b")]));
    fold.apply(ev("workflow.paused", { runId: "run-1", from: "a", port: "next", to: "b" }));

    expect(runWaiting(viewOf(fold), false, undefined)).toEqual({
      kind: "pause",
      to: "b",
      toLabel: "Review"
    });
  });

  it("returns a human waiting shape with filled question and choices", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [
          {
            id: "h",
            kind: "human",
            name: "Ask",
            question: "raw question",
            choices: ["Yes", "No"],
            position: { x: 0, y: 0 }
          }
        ],
        [edge("start", "next", "h")]
      )
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "h", pass: 1 }));
    fold.apply(ev("workflow.human.asked", { runId: "run-1", nodeId: "h", question: "filled question" }));

    expect(runWaiting(viewOf(fold), false, undefined)).toEqual({
      kind: "human",
      nodeId: "h",
      question: "filled question",
      choices: ["Yes", "No"]
    });
  });

  it("falls back to the raw human question when no asked event exists", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [
          {
            id: "h",
            kind: "human",
            name: "Ask",
            question: "raw question",
            choices: ["Go"],
            position: { x: 0, y: 0 }
          }
        ],
        [edge("start", "next", "h")]
      )
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "h", pass: 1 }));

    expect(runWaiting(viewOf(fold), false, undefined)).toEqual({
      kind: "human",
      nodeId: "h",
      question: "raw question",
      choices: ["Go"]
    });
  });

  it("returns undefined for an open agent node when lastDone is waiting_for_user", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(runWaiting(viewOf(fold), false, "waiting_for_user")).toBeUndefined();
  });

  it("returns interrupted for other open nodes", () => {
    const fold = createRunFold();
    started(fold, def([node("a", "agent", "A")], [edge("start", "next", "a")]));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(runWaiting(viewOf(fold), false, "complete")).toEqual({ kind: "interrupted" });
  });
});

describe("upcomingSteps", () => {
  it("lists the nominal next steps from the current open node", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [node("start", "start"), node("a", "agent", "A"), node("b", "agent", "B"), node("end", "end")],
        [edge("start", "next", "a"), edge("a", "done", "b"), edge("b", "next", "end")]
      )
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(upcomingSteps(viewOf(fold))).toEqual([{ nodeId: "b", label: "B" }]);
  });

  it("includes the paused target as the first upcoming step", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [node("start", "start"), node("a", "agent", "A"), node("b", "agent", "B"), node("end", "end")],
        [edge("start", "next", "a"), edge("a", "next", "b"), edge("b", "next", "end")]
      )
    );
    fold.apply(ev("workflow.paused", { runId: "run-1", from: "a", port: "next", to: "b" }));

    expect(upcomingSteps(viewOf(fold))).toEqual([{ nodeId: "b", label: "B" }]);
  });

  it("returns no upcoming steps for a finished run", () => {
    const fold = createRunFold();
    started(
      fold,
      def([node("a", "agent", "A"), node("b", "agent", "B")], [edge("a", "next", "b")])
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));
    fold.apply(ev("workflow.finished", { runId: "run-1", outcome: "done" }));

    expect(upcomingSteps(viewOf(fold))).toEqual([]);
  });

  it("stops at an end node without listing it", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [node("start", "start"), node("a", "agent", "A"), node("end", "end")],
        [edge("start", "next", "a"), edge("a", "next", "end")]
      )
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(upcomingSteps(viewOf(fold))).toEqual([]);
  });

  it("does not relist a node already seen in a loop", () => {
    const fold = createRunFold();
    started(
      fold,
      def([node("a", "agent", "A"), node("b", "agent", "B")], [
        edge("a", "next", "b"),
        edge("b", "next", "a")
      ])
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(upcomingSteps(viewOf(fold))).toEqual([{ nodeId: "b", label: "B" }]);
  });

  it("prefers a positive port when choosing the nominal edge", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [node("a", "agent", "A"), node("x", "agent", "X"), node("y", "agent", "Y")],
        [edge("a", "fail", "x"), edge("a", "done", "y")]
      )
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(upcomingSteps(viewOf(fold))).toEqual([{ nodeId: "y", label: "Y" }]);
  });

  it("uses the first non negative port when no positive port exists", () => {
    const fold = createRunFold();
    started(
      fold,
      def(
        [node("a", "agent", "A"), node("x", "agent", "X"), node("y", "agent", "Y")],
        [edge("a", "fail", "x"), edge("a", "custom", "y")]
      )
    );
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(upcomingSteps(viewOf(fold))).toEqual([{ nodeId: "y", label: "Y" }]);
  });

  it("returns no steps when the workflow has no nodes", () => {
    const fold = createRunFold();
    started(fold, def([], []));
    fold.apply(ev("workflow.node.started", { runId: "run-1", nodeId: "a", pass: 1 }));

    expect(upcomingSteps(viewOf(fold))).toEqual([]);
  });
});

describe("runRounds", () => {
  const run = (nodeId: string, pass = 1): NodeRun => ({
    nodeId,
    label: nodeId,
    kind: "agent",
    pass,
    state: "done"
  });

  it("returns an empty array for no runs", () => {
    expect(runRounds([])).toEqual([]);
  });

  it("keeps distinct nodes in one round", () => {
    expect(runRounds([run("a"), run("b"), run("c")])).toEqual([[run("a"), run("b"), run("c")]]);
  });

  it("starts a new round when a node id repeats", () => {
    expect(runRounds([run("a"), run("b"), run("a"), run("c")])).toEqual([
      [run("a"), run("b")],
      [run("a"), run("c")]
    ]);
  });

  it("starts a new round for every repeated node id", () => {
    expect(runRounds([run("a"), run("b"), run("a"), run("c"), run("a")])).toEqual([
      [run("a"), run("b")],
      [run("a"), run("c")],
      [run("a")]
    ]);
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
