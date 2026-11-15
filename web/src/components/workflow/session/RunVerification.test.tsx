import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { RunVerification } from "./RunVerification";
import type { Verification } from "../../../lib/api";

const emptyVerification: Verification = {
  runId: "run-1",
  criteria: [],
  testsProtected: 0,
  waiversAsked: 0,
  waiversGranted: 0,
  outsidePlan: null,
  block: null,
};

const fullVerification: Verification = {
  runId: "run-1",
  criteria: [
    {
      id: "c1",
      description: "Build passes",
      outcome: { kind: "verified", command: "npm run build", exitCode: 0, passed: true },
    },
    {
      id: "c2",
      description: "Tests pass",
      outcome: { kind: "verified", command: "npm test", exitCode: 1, passed: false },
    },
    {
      id: "c3",
      description: "Looks right",
      outcome: { kind: "judged", passed: true },
    },
    {
      id: "c4",
      description: "Waiting",
      outcome: { kind: "pending" },
    },
  ],
  testsProtected: 3,
  waiversAsked: 2,
  waiversGranted: 1,
  outsidePlan: ["src/a.ts", "src/b.ts"],
  block: "verification block",
};

describe("RunVerification", () => {
  it("renders the four outcome kinds with their badges", () => {
    const markup = renderToStaticMarkup(
      <RunVerification verification={fullVerification} />,
    );

    expect(markup).toContain("Verified by fennec");
    expect(markup).toContain("verified");
    expect(markup).toContain("failed");
    expect(markup).toContain("indicative");
    expect(markup).toContain("not checked");
  });

  it("shows the outside paths", () => {
    const markup = renderToStaticMarkup(
      <RunVerification verification={fullVerification} />,
    );

    expect(markup).toContain("src/a.ts");
    expect(markup).toContain("src/b.ts");
  });

  it("renders nothing for an empty verification", () => {
    expect(
      renderToStaticMarkup(<RunVerification verification={emptyVerification} />),
    ).toBe("");
  });
});
