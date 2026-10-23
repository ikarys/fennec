// node ui/card.test.js — the card never repeats the title, which the head
// says; a request for later is edited only once "Edit" is clicked.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];

class Node {
  constructor(tag) {
    this.tag = tag; this.children = []; this.textContent = ""; this.className = "";
    this.dataset = {}; this.style = { setProperty() {} }; this.classList = { add() {} }; this.parent = null;
  }
  append(...cs) { for (const c of cs) { if (c && typeof c === "object") c.parent = this; this.children.push(c); } }
  querySelector(sel) {
    const cls = sel.replace(":", "").split(",").map((s) => s.trim().replace(".", ""));
    const find = (n) => {
      if (cls.some((c) => n.className === c || (n.className || "").split(" ").includes(c))) return n;
      for (const ch of n.children) { const r = find(ch); if (r) return r; }
      return null;
    };
    return find(this);
  }
  querySelectorAll() { return []; }
  replaceWith(o) {
    if (!this.parent) return;
    const i = this.parent.children.indexOf(this);
    if (i < 0) return;
    this.parent.children.splice(i, 1);
    o.parent = this.parent;
    this.parent.children.splice(i, 0, o);
  }
  contains() { return false; }
  after() {}
  prepend(c) { c.parent = this; this.children.unshift(c); }
  focus() {}
}
global.document = { createElement: (t) => new Node(t), getElementById: () => new Node("div") };

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const TAGS = \{[\s\S]*?\n\};/)[0]
  + grab("landLabel") + grab("landTitle") + grab("firstLine")
  // card's collaborators, stubbed: they only shape the structure under test.
  + `
  const conversation = () => null;
  const askedOf = () => null;
  const thumbs = () => null;
  const attach = () => ({ node: new Node("div"), value: () => "" });
  const focus = () => new Node("div");
  const dropButton = () => new Node("button");
  const action = () => new Node("button");
  const changesButton = () => new Node("button");
  const codeButton = () => new Node("button");
  const pageButton = () => new Node("button");
  const journalButton = () => new Node("button");
  const details = () => { const n = new Node("details"); n.className = "more"; return n; };
  const work = () => [];
  const shots = () => null;
  const facts = () => [];
  const doneWhen = () => null;
  const rich = () => [new Node("span")];
  const explain = (r) => r;
  let saved = "";
  `
  + grab("card")
  + "globalThis.card = card;");

// A ready request: its title and status live in the head, not the card.
const ready = card({ id: "r", title: "une page", kind: "ready", project: "p" });
assert.ok(!ready.children.some((c) => (c.className || "").includes("title")), "plus de .title sur une demande prête");
assert.ok(!ready.children.some((c) => (c.className || "").includes("meta")), "plus de .meta : le status est dans l'en-tête");

// A later request: read, then edited once "Edit" is clicked.
const later = card({ id: "l", title: "plus tard", kind: "later", project: "p" });
assert.ok(!later.querySelector(".later-text"), "pas de champ avant « Éditer »");
const find = (n, f) => f(n) ? n : n.children?.map((c) => find(c, f)).find(Boolean);
find(later, (n) => n.textContent === "Edit").onclick();
assert.ok(later.querySelector(".later-text"), "« Éditer » ouvre le champ");

// A card that asks a question: the decision buttons come before the stats,
// so the answer does not wait for the figures.
const blocked = card({ id: "b", title: "une question", kind: "blocked", project: "p", question: "quel choix ?" });
const actionsIdx = blocked.children.findIndex((c) => (c.className || "").includes("actions"));
const detailsIdx = blocked.children.findIndex((c) => (c.className || "").includes("more"));
assert.ok(actionsIdx >= 0 && detailsIdx >= 0 && actionsIdx < detailsIdx, "les boutons de décision précèdent les stats");

console.log("OK");
