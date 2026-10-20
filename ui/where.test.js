// node ui/where.test.js — the address round-trips: "#<project>/<request>".
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";
const region = src.match(/const where = \(\) => \{[\s\S]*?\n\};\nconst hashOf = [\s\S]*?;/)[0];

global.location = { hash: "" };
eval(region + "\nglobalThis.hashOf = hashOf; globalThis.where = where;");

// The app writes percent-encoded paths: the request must come back as the id.
const url = hashOf("/home/tony/workspace/x/proj", "000000000000000018df947d4b0a4beb");
location.hash = url;
assert.deepStrictEqual(where(), { path: "/home/tony/workspace/x/proj", id: "000000000000000018df947d4b0a4beb" });

// A project without a request: the id is empty, not folded into the path.
location.hash = hashOf("/home/tony/workspace/x/proj", "");
assert.deepStrictEqual(where(), { path: "/home/tony/workspace/x/proj", id: "" });

// A discussion: its address is plain and already worked.
location.hash = "#chat/6ad48afa2913b";
assert.deepStrictEqual(where(), { path: "chat", id: "6ad48afa2913b" });
location.hash = "#chat";
assert.deepStrictEqual(where(), { path: "chat", id: "" });

// The pages the address names: team, stats, settings.
for (const name of ["team", "stats", "settings"]) {
  location.hash = "#" + name;
  assert.deepStrictEqual(where(), { path: name, id: "" });
}

// No address at all.
location.hash = "";
assert.deepStrictEqual(where(), { path: "", id: "" });

// Hand-typed raw slashes: the last one still names the request.
location.hash = "#/home/tony/workspace/x/proj/abc123";
assert.deepStrictEqual(where(), { path: "/home/tony/workspace/x/proj", id: "abc123" });

// A leftover one-segment fragment is not a project.
location.hash = "#/home";
assert.deepStrictEqual(where(), { path: "", id: "home" });
console.log("ok");
