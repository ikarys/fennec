// node ui/nav.test.js — leaving a page (team / stats / settings) or a
// discussion by choosing a project: the address must return to the project,
// otherwise the page stays on screen and the requests bar never comes back.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");

eval(src.match(/const PAGES = \[[^\n]*\n/)[0]
  + src.match(/const owned = [^\n]*\n/)[0]
  + "globalThis.owned = owned;");

assert.equal(owned("chat"), true, "a discussion owns the address");
assert.equal(owned("team"), true, "the team page owns the address");
assert.equal(owned("stats"), true, "the stats page owns the address");
assert.equal(owned("settings"), true, "the settings page owns the address");
assert.equal(owned(""), false, "no path: the project's page owns the address");
assert.equal(owned("/home/tony/workspace/tests"), false, "a project path does not own it");

// The select's change handler takes the address back through `owned`, not only
// for a discussion.
assert.ok(/addEventListener\("change", \(\) => \{[\s\S]*?owned\(where\(\)\.path\)/.test(src),
  "the project change handler asks `owned` before taking the address back");

console.log("ok nav.test.js");
