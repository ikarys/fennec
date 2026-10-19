// node ui/chat.test.js — a turn that waits on the person's answer shows its
// question where the person looks: at the end of the exchange, not buried
// under it.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];

// A fake DOM just wide enough for the chat pane: what goes in comes out in order.
class Node {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.textContent = "";
    this.scrollTop = 0;
    this.scrollHeight = 0;
    this.clientHeight = 0;
  }
  append(...cs) { this.children.push(...cs); }
  prepend(...cs) { this.children.unshift(...cs); }
  replaceChildren(...cs) { this.children = cs; }
  querySelectorAll() { return []; }
  setAttribute() {}
  removeAttribute() {}
  focus() {}
}
const byId = {};
global.document = {
  createElement: (t) => new Node(t),
  getElementById: (n) => (byId[n] ??= new Node(n)),
};

const text = (n) => (typeof n === "string" ? n : n.textContent ?? "") + (n.children ?? []).map(text).join("");
eval("let chatBox = null; let detailed = '';"
  + src.match(/const \$ = \(id\) => document\.getElementById\(id\);/)[0]
  + src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + "const setIcon = () => {};"
  + "const metrics = () => {}; const openPicker = () => {}; const thumbs = () => null;"
  + "const attach = (t) => ({ node: null, value: () => t.value, clear: () => { t.value = ''; } });"
  + grab("isImage")
  + grab("inline") + grab("rich") + grab("md")
  + grab("chatTitle") + grab("toolCall") + grab("chatThread")
  + grab("chatQuestion") + grab("chatComposer") + grab("chatAside") + grab("showChat")
  + "globalThis.showChat = showChat; globalThis.chatThread = chatThread; globalThis.getChatBox = () => chatBox;");

const base = {
  id: "abc1234567890",
  project: true,
  folder: "/home/tony/workspace/games/incr-ia",
  grants: [],
  stats: null,
  messages: [
    { role: "user", content: "and now ?" },
    { role: "assistant", content: "", tool_calls: [
      { id: "c1", function: { name: "shell", arguments: "{\"cmd\": \"ls\"}" } }
    ] },
    { role: "tool", tool_call_id: "c1", content: "exit 0" },
  ],
};

// A turn blocked on the person: the question and its marker sit at the end
// of the exchange, not on top of it.
showChat({ ...base, status: "waiting",
  pending: { tool: "shell", path: "/home/tony/workspace/games", directory: "/home/tony/workspace/games" } });
let thread = getChatBox().thread;
let ask = thread.children.filter((n) => (n.className ?? "").includes("ask"));
assert.equal(ask.length, 1, "the pending question is drawn");
assert.notEqual(thread.children[0], ask[0], "the question is not buried on top of the exchange");
assert.equal(thread.children.at(-2), ask[0], "the question is last but one, next to the marker");
assert.ok((thread.children.at(-1).className ?? "").includes("thinking"), "a marker sits at the very bottom");
assert.equal(text(thread.children.at(-1)), "fennec attend ta réponse");
assert.equal(getChatBox().composer.send.disabled, true, "send stays off while the turn waits");
assert.equal(getChatBox().composer.stop.hidden, false, "staying can still be asked");

// A running turn: its marker stays at the bottom, no question.
showChat({ ...base, status: "thinking" });
thread = getChatBox().thread;
assert.ok(thread.children.every((n) => !(n.className ?? "").includes("ask")));
assert.equal(text(thread.children.at(-1)), "fennec répond…");

// A finished turn: no question, no marker.
showChat({ ...base, status: "idle" });
thread = getChatBox().thread;
assert.ok(!thread.children.some((n) => (n.className ?? "").includes("ask")));
assert.ok(!thread.children.some((n) => (n.className ?? "").includes("thinking")));
console.log("ok");

// Saying a message again: its text goes back in the composer, and the index
// sent is the message's own in the exchange.
{
  const seen = [];
  const nodes = chatThread({ ...base, messages: [...base.messages, { role: "user", content: "encore" }] },
    (i, content) => seen.push([i, content]));
  const human = nodes.filter((n) => n.className === "said human");
  human.at(-1).children[0].children[1].onclick();
  assert.deepEqual(seen, [[3, "encore"]]);
}
