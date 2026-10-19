// node ui/stop.test.js — a request under way can be stopped: the row offers
// a stop icon (one click more to confirm), the detail view offers "Arrêter",
// both call /api/stop, and the old row actions keep working.
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
  setAttribute(k, v) { this[k] = v; }
  replaceWith(other) { this.replacedBy = other; }
  closest() { return { querySelector: () => new Node("div") }; }
}
global.document = {
  createElement: (t) => new Node(t),
  getElementById: () => new Node("div"),
};
global.refresh = () => {};
const calls = [];
global.fetch = async (path, opts) => {
  calls.push([path, opts && JSON.parse(opts.body)]);
  return { ok: true, json: async () => ({}) };
};

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICONS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICON_OF = \[\[[\s\S]*?\];\n/)[0]
  + grab("setIcon") + grab("iconize")
  + src.match(/const errorOf = \(button\) => [^\n]*\n/)[0]
  + src.match(/async function api\([\s\S]*?\n}\n/)[0]
  + grab("action") + grab("line") + grab("headline") + grab("rowActions")
  + grab("underway") + src.match(/const STEPS = \[[\s\S]*?\]\];\n/)[0] + grab("stages") + grab("ended") + grab("conversation") + grab("doneWhen") + grab("facts")
  + "globalThis.rowActions = rowActions; globalThis.underway = underway;");

// The row of a running request: a stop icon, one click more to confirm.
const row = rowActions("r1", { stop: "/api/stop" });
assert.equal(row.children.length, 2, "stop + error, ni archive ni suppression");
const halt = row.children[0];
assert.equal(halt.className, "icon");
assert.equal(halt.ariaLabel, "Arrêter");
halt.onclick();
assert.equal(calls.length, 0, "le premier clic ne fait que demander");
const sure = halt.replacedBy;
assert.equal(sure.textContent, "Sûr ?");
sure.onclick().then(() => {
  assert.deepEqual(calls, [["/api/stop", { id: "r1" }]], "le second clic arrête");

  // The old rows keep their own actions.
  const old = rowActions("r2", { archive: "/api/request/archive", forget: "/api/forget" });
  assert.equal(old.children.length, 3);
  assert.equal(old.children[0].ariaLabel, "Archiver");
  assert.equal(old.children[1].ariaLabel, "Supprimer");
  old.children[1].onclick();
  assert.equal(old.children[1].replacedBy.textContent, "Confirmer");

  // The detail view of a running request offers "Arrêter".
  const box = underway({ id: "u1", step: "travaille", since: 100, title: "x" });
  const bar = box.children.find((n) => n.className?.includes("run-actions"));
  assert.ok(bar.className.includes("run-actions"), "la barre d'arrêt est sous la ligne");
  const stop = bar.children[0];
  assert.equal(stop.textContent, "Arrêter");
  stop.onclick();
  const confirm = stop.replacedBy;
  assert.equal(confirm.textContent, "Confirmer");
  confirm.onclick().then(() => {
    assert.ok(calls.some(([p, b]) => p === "/api/stop" && b.id === "u1"), "le détail arrête aussi");
    console.log("ok");
  }).catch((e) => { console.error(e); process.exit(1); });
}).catch((e) => { console.error(e); process.exit(1); });
