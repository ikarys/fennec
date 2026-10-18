// node ui/grant.test.js — the question fennec asks about a path outside the
// folder, and the person's answer.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const region = src.match(/function chatQuestion\(c\) \{[\s\S]*?\n\}\n/)[0];

const nodes = [];
globalThis.el = (tag, props = {}, ...children) => {
  const node = Object.assign({ tag, children: [], parent: null }, props);
  node.append = (...c) => c.forEach((k) => {
    if (k === null || k === undefined || k === false) return;
    node.children.push(k);
    k.parent = node;
  });
  node.append(...children);
  nodes.push(node);
  return node;
};
const calls = [];
globalThis.api = async (path, body) => { calls.push([path, body]); };
globalThis.$ = () => ({ textContent: "" });

eval(region);

const c = { id: "abc", pending: {
  tool: "shell", path: "/home/t/other/o.txt", directory: "/home/t/other" } };

// The card names the path as written and the folder that would open.
const card = chatQuestion(c);
assert.strictEqual(card.className, "ask");
const byText = (t) => nodes.find((n) => n.textContent === t);
assert.strictEqual(byText("fennec demande").tag, "span");
assert.strictEqual(byText("/home/t/other/o.txt").tag, "code");
assert.strictEqual(byText("/home/t/other").parent, card);
const open = byText("Ouvrir le dossier");
const refuse = byText("Refuser");
assert.strictEqual(open.className, "primary");
assert.strictEqual(refuse.className, "quiet");
assert.strictEqual(open.parent, card);
assert.strictEqual(refuse.parent, card);

// The buttons are the person's answer, and nothing else reaches the server.
Promise.all([open.onclick(), refuse.onclick()]).then(() => {
  assert.deepStrictEqual(calls, [
    ["/api/chat/decide", { id: "abc", allow: true }],
    ["/api/chat/decide", { id: "abc", allow: false }],
  ]);
  console.log("ok");
});
