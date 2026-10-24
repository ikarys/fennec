// node ui/compose.test.js — the compose form is short input for the task
// list: it must not sit over a page (team / stats / settings), where its
// project choice would take the address back from the page and shadow the
// page's own project filter.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";

let modalShown = 0;
const node = () => ({ textContent: "", value: "", focus() {}, showModal() { modalShown++; } });
const ids = new Map();
globalThis.location = { hash: "#stats" };
global.document = { getElementById: (id) => (ids.has(id) ? ids.get(id) : (ids.set(id, node()), ids.get(id))) };

eval(src.match(/const PAGES = \[[^\n]*\n/)[0]
  + src.match(/const where = \(\) => \{[\s\S]*?\n\};\n/)[0]
  + src.match(/const \$ = [^\n]*\n/)[0]
  + src.match(/function compose\(\) \{[\s\S]*?\n}\n/)[0]
  + "globalThis.compose = compose;");

// On a page: the form does not open, so its "All projects" cannot shadow the
// page's own filter.
compose();
assert.equal(modalShown, 0, "on the Stats page the compose form stays closed");
for (const page of ["#team", "#settings", ""]) {
  location.hash = page;
  compose();
}
assert.equal(modalShown, 1, "off the pages (task list) the compose form opens");

// Entering a page closes a form left open: showDetail says so.
assert.ok(
  /if \(page\) \{\n\s*\/\/[^\n]*\n\s*\/\/[^\n]*\n\s*\$\(\"compose\"\)\.close\(\);/.test(src),
  "showDetail closes the compose form when a page is shown"
);

console.log("ok compose.test.js");
