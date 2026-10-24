// node ui/stats.test.js — the Stats page's filters (period, project) ride on
// the query string, and the answer's by-day and per-task lines are drawn.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";
globalThis.Option = class Option {
  constructor(text, value) { this.text = text; this.value = value; }
};

class Node {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.textContent = "";
    this.className = "";
    this.options = [];
    this.value = "";
    this.style = { setProperty() {} };
  }
  append(...cs) {
    this.children.push(...cs);
    this.options.push(...cs.filter((c) => c instanceof Option));
  }
  replaceChildren(...cs) { this.children = cs; }
}
const nodes = {};
global.document = {
  createElement: (t) => new Node(t),
  getElementById: (id) => (nodes[id] ??= new Node("div")),
};
nodes["stats-range"] = new Node("select");
nodes["stats-range"].value = "7";
nodes["stats-project"] = new Node("select");
nodes["stats-project"].value = "/p";

const full = {
  all: { requests: 2, ms: 2000, sent: 300, cached: 150, received: 20 },
  projects: [{ path: "/p", name: "p", place: "here", requests: 1, landed: 1, failed: 0, failed_attempts: 0, ms: 1000, sent: 100 }],
  models: [{ name: "a", turns: 1, sent: 100, cached: 50, received: 10, ms: 1000 }],
  stages: [{ name: "Work", turns: 1, ms: 1000, sent: 100, received: 10 }],
  by_day: [{ day: "2026-02-10", sent: 100, received: 10 }],
  tasks: [{ id: "1", title: "recent", project: "p", models: "a", sent: 100, cached: 50, received: 10, ms: 1000, since: 1 }],
};
const filtered = { ...full, all: { requests: 1, ms: 1000, sent: 100, cached: 50, received: 10 } };
const calls = [];
global.fetch = (url) => {
  calls.push(url);
  return Promise.resolve({ ok: true, json: async () => (url === "/api/stats" ? full : filtered) });
};

eval(src.match(/const \$ = [^\n]*\n/)[0]
  + src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/async function api\([\s\S]*?\n}\n/)[0]
  + src.match(/const LEGACY_STEPS = [\s\S]*?\];\n/)[0]
  + src.match(/const stageKey = \([\s\S]*?\n};\n/)[0]
  + src.match(/const stageRank = [^\n]*\n/)[0]
  + src.match(/const stageName = \([\s\S]*?\n};\n/)[0]
  + src.match(/const number = [^\n]*\n/)[0]
  + src.match(/const compact = [^\n]*\n/)[0]
  + src.match(/const percent = [^\n]*\n/)[0]
  + src.match(/const tps = [^\n]*\n/)[0]
  + src.match(/const speed = [^\n]*\n/)[0]
  + src.match(/function duration\(ms\) \{[\s\S]*?\n}\n/)[0]
  + src.match(/const PLACES = [^\n]*\n/)[0]
  + src.match(/const cell = [^\n]*\n/)[0]
  + src.match(/async function showStats\(\) \{[\s\S]*?\n}\n/)[0]
  + "globalThis.showStats = showStats;");

(async () => {
  await showStats();
  assert.equal(calls[0], "/api/stats", "the project list, once, unfiltered");
  assert.equal(calls[1], "/api/stats?days=7&project=%2Fp", "the filters ride on the query string");
  assert.deepEqual(
    nodes["stats-project"].options.map((o) => o.text),
    ["All projects", "p"],
    "the project select: all, then each project"
  );
  const days = nodes["stats-days"].children;
  assert.equal(days.length, 1, "one line per day");
  assert.equal(days[0].children[0].textContent, "2026-02-10", "the day, as the server names it");
  const tasks = nodes["stats-tasks"].children;
  assert.equal(tasks.length, 1, "one line per task");
  assert.equal(tasks[0].children[0].textContent, "recent", "the task's title");
  assert.equal(tasks[0].children[1].textContent, "p", "its project");
  assert.equal(tasks[0].children[2].textContent, "a", "its models");
  // The list is already there: the next look sends only the filtered query.
  await showStats();
  assert.equal(calls.length, 3, "no second unfiltered look");
  assert.equal(calls[2], "/api/stats?days=7&project=%2Fp");
  console.log("ok stats.test.js");
})();
