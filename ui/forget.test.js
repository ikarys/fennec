// node ui/forget.test.js — erasing a finished request asks once before doing.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];

class Node {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.textContent = "";
    this.className = "";
    this.style = { setProperty() {} };
    this.classList = { add() {} };
  }
  append(...cs) { this.children.push(...cs); }
  replaceWith(other) { this.replacedBy = other; }
  setAttribute() {}
  closest() { return { querySelector: () => new Node("div") }; }
}
global.document = {
  createElement: (t) => new Node(t),
  getElementById: () => new Node("div"),
};
const calls = [];
global.fetch = async (path, opts) => {
  calls.push([path, JSON.parse(opts.body)]);
  return { ok: true, json: async () => ({}) };
};
global.refresh = () => {};

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICONS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICON_OF = \[\[[\s\S]*?\];\n/)[0]
  + grab("setIcon") + grab("iconize")
  + src.match(/const errorOf = \(button\) => [^\n]*\n/)[0]
  + src.match(/async function api\([\s\S]*?\n}\n/)[0] + grab("action") + grab("forgetButton")
  + "globalThis.forgetButton = forgetButton;");

const b = forgetButton("abc");
assert.equal(b.textContent, "Supprimer");
assert.ok((b.className).includes("quiet"));
b.onclick();
assert.equal(calls.length, 0, "le premier clic ne fait que demander");
const sure = b.replacedBy;
assert.equal(sure.textContent, "Confirmer la suppression");
sure.onclick().then(() => {
  assert.deepEqual(calls, [["/api/forget", { id: "abc" }]], "le second clic efface");
  console.log("ok");
}).catch((e) => { console.error(e); process.exit(1); });
