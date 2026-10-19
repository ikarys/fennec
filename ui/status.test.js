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
  setAttribute(k, v) { this[k] = v; }
}
global.document = {
  createElement: (t) => new Node(t),
  body: { classList: { contains: () => false } },
};

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const TAGS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICONS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICON_OF = \[[\s\S]*?\];/)[0]
  + grab("pastState") + grab("duration") + arrow("compact")
  + grab("setIcon") + grab("statusOf") + grab("reqHead")
  + grab("iconize") + arrow("errorOf") + grab("action") + src.match(/async function follow\([\s\S]*?\n\}\n/)[0] + grab("underway") + src.match(/const STEPS = \[[\s\S]*?\]\];\n/)[0] + grab("stages") + grab("ended") + grab("conversation") + grab("doneWhen") + grab("facts")
  + "globalThis.statusOf = statusOf; globalThis.reqHead = reqHead; globalThis.underway = underway;");
global.api = async () => [];
global.refresh = () => {};

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
assert.deepEqual(statusOf({ waiting: true }, "run"), ["en attente", "muted"], "en file, pas en cours");

// The head: the title in whole, the coloured pill beside it.
const item = { id: "x", title: "rend l'interface plus jolie", kind: "passed", stats: { turns: 1, ms: 1000, sent: 10, received: 5 } };
const head = reqHead(item, "past");
assert.equal(head.children[0].tag, "strong");
assert.equal(head.children[0].textContent, "rend l'interface plus jolie", "le titre, en entier");
assert.equal(head.children[1].className, "status warn", "le status coloré");
assert.equal(head.children[1].textContent, "mis de côté");
assert.ok(head.children[2].textContent.includes("⏱"), "le coût, pour le passé");

// A running request: no cost, an "en cours" pill, the right bar's toggle.
const run = reqHead({ id: "r", title: "t" }, "run");
assert.equal(run.children[1].className, "status info");
assert.equal(run.children.length, 3, "pas de coût sur ce qui tourne");
assert.equal(run.children[2].id, "toggle-facts", "le toggle de la barre de droite");

// Work under way: "Arrêter" always; "Réessayer" only when the worker can be
// dead — queued, or mid-commit.
const barOf = (box) => box.children.find((n) => n.className === "past-actions run-actions");
const busy = { id: "w", title: "t", kind: "run", step: "travaille", since: 1000 };
assert.ok(barOf(underway(busy)).children.some((b) => b.textContent === "Arrêter"));
assert.ok(!barOf(underway(busy)).children.some((b) => b.textContent === "Réessayer"), "rien à réessayer tant que ça tourne");
assert.ok(barOf(underway({ ...busy, waiting: true, step: "attend son tour : le dossier a des changements pas encore commités" }))
  .children.some((b) => b.textContent === "Réessayer"), "en attente : on peut réessayer");
assert.ok(barOf(underway({ ...busy, step: "fait le commit" }))
  .children.some((b) => b.textContent === "Réessayer"), "en train du commit : on peut réessayer");
assert.ok(!barOf(underway({ ...busy, waiting: true, alive: true }))
  .children.some((b) => b.textContent === "Réessayer"), "en attente, worker vivant : rien à réessayer");

// Where the work stands, as steps: before it done, it highlighted.
const steps = underway(busy).children[0].children;
assert.deepEqual(steps.map((li) => li.className), ["done", "done", "now", "", ""], "travaille = Travail en cours");

// Finished: what the figures show reached, and where it broke.
const failed = ended({ kind: "failed", stats: { stages: { Discussion: {}, Travail: {}, Preuve: {} } } }).children;
assert.deepEqual(failed.map((li) => li.className), ["done", "", "done", "stop", ""], "échoué à l'essai, sans cadrage");
const landed = ended({ kind: "landed", stats: { stages: { Discussion: {}, Travail: {}, Preuve: {} } } }).children;
assert.equal(landed.at(-1).className, "done", "commité : Commit fait");

console.log("OK");
