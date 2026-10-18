// node ui/status.test.js — the request's head: the title in whole above the
// tabs, one coloured pill for where it stands; every status has a tone.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];
const arrow = (name) => src.match(new RegExp(`const ${name} = [^\\n]*\\n`))[0];

class Node {
  constructor(tag) {
    this.tag = tag; this.children = []; this.textContent = ""; this.className = "";
    this.dataset = {}; this.style = { setProperty() {} }; this.classList = { add() {} };
  }
  append(...cs) { this.children.push(...cs); }
}
global.document = { createElement: (t) => new Node(t) };

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const TAGS = \{[\s\S]*?\n\};/)[0]
  + grab("pastState") + grab("duration") + arrow("compact")
  + grab("statusOf") + grab("reqHead")
  + "globalThis.statusOf = statusOf; globalThis.reqHead = reqHead;");

// Every status, in every view, has a label and a tone.
assert.deepEqual(statusOf({ kind: "talk" }, "todo"), ["Discussion", "accent"]);
assert.deepEqual(statusOf({ kind: "validate" }, "todo"), ["À valider", "accent"]);
assert.deepEqual(statusOf({ kind: "blocked" }, "todo"), ["Question", "accent"]);
assert.deepEqual(statusOf({ kind: "rule" }, "todo"), ["Règle ?", "accent"]);
assert.deepEqual(statusOf({ kind: "ready" }, "todo"), ["Prêt", "ok"]);
assert.deepEqual(statusOf({ kind: "later" }, "todo"), ["Plus tard", "muted"]);
assert.deepEqual(statusOf({ kind: "failed" }, "todo"), ["Échoué", "bad"]);
assert.deepEqual(statusOf({ kind: "dropped" }, "past"), ["abandonné", "bad"]);
assert.deepEqual(statusOf({ kind: "passed" }, "past"), ["mis de côté", "warn"]);
assert.deepEqual(statusOf({ kind: "landed", branch: "main", commit: "abc1234" }, "past"), ["✓ commit main · abc1234", "ok"]);
assert.deepEqual(statusOf({}, "run"), ["en cours", "info"]);

// The head: the title in whole, the coloured pill beside it.
const item = { id: "x", title: "rend l'interface plus jolie", kind: "passed", stats: { turns: 1, ms: 1000, sent: 10, received: 5 } };
const head = reqHead(item, "past");
assert.equal(head.children[0].tag, "strong");
assert.equal(head.children[0].textContent, "rend l'interface plus jolie", "le titre, en entier");
assert.equal(head.children[1].className, "status warn", "le status coloré");
assert.equal(head.children[1].textContent, "mis de côté");
assert.ok(head.children[2].textContent.includes("⏱"), "le coût, pour le passé");

// A running request: no cost, an "en cours" pill.
const run = reqHead({ id: "r", title: "t" }, "run");
assert.equal(run.children[1].className, "status info");
assert.equal(run.children.length, 2, "pas de coût sur ce qui tourne");

console.log("OK");
