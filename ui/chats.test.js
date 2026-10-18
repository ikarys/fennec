// node ui/chats.test.js — the discussion list is grouped by folder: one
// section per place, projects marked, the most recent first within a folder,
// folders in the order of their names — not of their paths. Each section is
// a button, folded at first with its count; each row offers archiving and,
// one click more, erasing.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];

class Node {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.textContent = "";
    this.dataset = {};
    this.className = "";
    this.style = { setProperty() {} };
    this.classList = { add() {} };
  }
  append(...cs) { this.children.push(...cs); }
  replaceChildren(...cs) { this.children = cs; }
  setAttribute(k, v) { this[k] = v; }
  replaceWith(other) { this.replacedBy = other; }
}
global.document = {
  createElement: (t) => new Node(t),
  getElementById: () => new Node("div"),
};
global.location = { hash: "" };
global.refresh = () => {};
const calls = [];
global.fetch = async (path, opts) => {
  calls.push([path, opts && JSON.parse(opts.body)]);
  return { ok: true, json: async () => ({}) };
};

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const where = \(\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICONS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const TAGS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const chatOpen = new Set\(\);/)[0]
  + src.match(/async function api\([\s\S]*?\n}\n/)[0]
  + grab("setIcon") + grab("line") + grab("rowActions")
  + src.match(/const rowWrap = \(link, actions\) => [^\n]*\n/)[0]
  + grab("chatRow") + grab("chatGroup") + grab("chatGroups")
  + "globalThis.chatGroups = chatGroups; globalThis.rowActions = rowActions;");

// The server lists the discussions newest first; the groups keep that order.
// "zzz" exists twice, in two trees: the name orders the sections, the path
// breaks the tie.
const chats = [
  { id: "c1", folder: "/home/tony/work/zzz", title: "dans zzz (work)", since: 300, status: "idle", project: false },
  { id: "a3", folder: "/home/tony/work/a", title: "récente dans a", since: 200, status: "thinking", project: true },
  { id: "a2", folder: "/home/tony/work/a", title: "ancienne dans a", since: 100, status: "waiting", project: true },
  { id: "z0", folder: "/home/tony/aa/zzz", title: "dans zzz (aa)", since: 50, status: "idle", project: false },
];
const groups = chatGroups(chats);
assert.equal(groups.length, 3, "une section par dossier");
const head = groups[0].children[0];
assert.equal(head.tag, "button", "l'en-tête de section est un bouton");
assert.equal(head["aria-expanded"], "false", "la section na repliée");
assert.equal(head.title, "/home/tony/work/a", "le nom du dossier ordonne les sections, pas le chemin");
assert.equal(head.children[0].textContent, "a", "la section porte le nom du dossier");
assert.equal(head.children[1].textContent, "projet", "un projet enregistré est marqué");
assert.equal(head.children[2].textContent, 2, "le compteur dit combien de discussions");
const body = groups[0].children[1];
assert.equal(body.hidden, true, "repliée par défaut");
head.onclick();
assert.equal(body.hidden, false, "un clic déplie");
assert.equal(head["aria-expanded"], "true");
head.onclick();
assert.equal(body.hidden, true, "un second clic replie");
head.onclick();
// The list is redrawn on every refresh: the opened sections must survive it.
const again = chatGroups(chats);
assert.equal(again[0].children[1].hidden, false, "un re-rendu garde la section dépliée");
assert.equal(again[0].children[0]["aria-expanded"], "true");
again[0].children[0].onclick();
assert.equal(chatGroups(chats)[0].children[1].hidden, true, "et repliée si repliée");
assert.equal(groups[1].children[0].title, "/home/tony/aa/zzz", "même nom : le chemin départage");
assert.equal(groups[2].children[0].title, "/home/tony/work/zzz");
assert.equal(groups[1].children[0].children.length, 2, "un simple dossier n'est pas marqué : nom, compteur");
assert.equal(groups[0].children[1].children[0].children[0].href, "#chat/a3", "la plus récente d'abord dans la section");
assert.equal(groups[0].children[1].children[1].children[0].href, "#chat/a2");
assert.equal(groups[1].children[1].children[0].children[0].href, "#chat/z0");
assert.equal(groups[2].children[1].children[0].children[0].href, "#chat/c1");
assert.equal(groups[0].children[1].children[0].children[1].className, "row-actions", "chaque ligne offre archiver et supprimer");
// A discussion says no state on the right of its title: it is noise there.
const statesOf = (row) => {
  const all = [];
  const walk = (n) => { all.push(n); if (n && n.children) n.children.forEach(walk); };
  walk(row.children[0].children[0]);
  return all.filter((c) => c.className === "state" || String(c.className).startsWith("status"));
};
assert.equal(statesOf(groups[0].children[1].children[1]).length, 0, "aucun état sur la ligne");
assert.equal(statesOf(groups[2].children[1].children[0]).length, 0, "ni sur la ligne d'un dossier simple");
// The one under way keeps its clock, alone: no state, no leading separator.
const thinking = groups[0].children[1].children[0].children[0].children[0];
const stateWrap = thinking.children.find((c) => c.className === "state");
assert.ok(stateWrap, "la ligne en cours garde son état");
assert.equal(stateWrap.children.length, 1, "seule l'horloge, pas de « · » devant");
assert.equal(stateWrap.children[0].className, "elapsed");
assert.equal(calls.length, 0);
const row = groups[0].children[1].children[0];
row.children[1].children[0].onclick().then(() => {
  assert.deepEqual(calls, [["/api/chat/archive", { id: "a3", on: true }]], "archiver la discussion");
  const del = row.children[1].children[1];
  del.onclick();
  assert.equal(calls.length, 1, "le premier clic sur supprimer ne fait que demander");
  return del.replacedBy.onclick();
}).then(() => {
  assert.deepEqual(calls.at(-1), ["/api/chat/forget", { id: "a3" }], "le second clic efface");
  return rowActions("x", { archive: "/api/request/archive", forget: "/api/forget", restore: true })
    .children[0].onclick();
}).then(() => {
  assert.deepEqual(calls.at(-1), ["/api/request/archive", { id: "x", on: false }], "« rétablir » dé-archive");
  console.log("ok");
}).catch((e) => { console.error(e); process.exit(1); });
