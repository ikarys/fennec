// node ui/heads.test.js — the project on screen is said once, at the top of
// the list and of the request read in full; hidden when none is chosen, in a
// chat, and on the pages that name themselves.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];

class Node {
  constructor(tag) {
    this.tag = tag;
    this.hidden = false;
    this.textContent = "";
  }
  querySelector(sel) {
    if (sel === "strong") return this.strong = this.strong || new Node("strong");
    if (sel === ".muted") return this.m = this.m || new Node("span");
    return null;
  }
}
const ids = {};
global.document = { getElementById: (id) => (ids[id] = ids[id] || new Node("div")) };
global.location = { hash: "" };
const project = new Node("select");
ids["project"] = project;

eval("const $ = (id) => document.getElementById(id); "
  + src.match(/const where = \(\) => \{[\s\S]*?\n\};/)[0]
  + grab("sayProject") + grab("heads") + grab("detailHead")
  + "globalThis.heads = heads; globalThis.detailHead = detailHead;");
const PATH = "/home/tony/workspace/ux-todo";
const pick = () => { project.value = PATH; project.selectedOptions = [{ text: "ux-todo" }]; };
const head = (id) => ({ hidden: ids[id].hidden, name: ids[id].strong?.textContent, path: ids[id].m?.textContent });

// A project on screen: the list says it; a request read in full says it too.
global.location.hash = "#" + encodeURIComponent(PATH);
pick();
heads();
detailHead({ id: "r1" });
assert.equal(head("list-head").hidden, false, "la liste dit le projet");
assert.equal(head("list-head").name, "ux-todo");
assert.equal(head("list-head").path, PATH);
assert.equal(head("detail-head").hidden, false, "la demande lue en entier dit le projet");
assert.equal(head("detail-head").name, "ux-todo");
assert.equal(head("detail-head").path, PATH);

// No project chosen: nothing to say, list and detail.
project.value = "";
project.selectedOptions = [];
heads();
detailHead({ id: "r1" });
assert.equal(head("list-head").hidden, true);
assert.equal(head("detail-head").hidden, true);

// A chat owns the page: the project's head would name the wrong context.
global.location.hash = "#chat/abc123";
pick();
heads();
assert.equal(head("list-head").hidden, true);

// A page that names itself (project page, team, stats, settings): no
// request read in full, so no detail head.
global.location.hash = "#team";
pick();
heads();
detailHead(null);
assert.equal(head("list-head").hidden, false);
assert.equal(head("detail-head").hidden, true);

console.log("OK");
