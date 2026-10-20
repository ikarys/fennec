// node ui/i18n.test.js — every text app.js passes to `t` as a literal, and
// every `data-t` text, placeholder, title and aria-label of index.html, has its French in fr.js.
const assert = require("assert");
const fs = require("fs");
const src = fs.readFileSync(__dirname + "/app.js", "utf8");
const html = fs.readFileSync(__dirname + "/index.html", "utf8");
eval(fs.readFileSync(__dirname + "/fr.js", "utf8").replace("const FR", "globalThis.FR"));
const said = [...src.matchAll(/\bt\("((?:[^"\\]|\\.)*)"/g)].map((m) => JSON.parse(`"${m[1]}"`));
const unescape = (s) => s.replace(/&quot;/g, "\"").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">");
// Attributes are translated as written; a text's spaces are folded first.
const marked = [...[...html.matchAll(/<\w+[^>]*\sdata-t(?:\s[^>]*)?>([^<]+)</g)].map((m) => m[1].trim().replace(/\s+/g, " ")),
  ...[...html.matchAll(/\s(?:placeholder|title|aria-label)="([^"]+)"/g)].map((m) => m[1])].map(unescape);
// Names and addresses, the same in every language.
const same = ["Fleet", "Stats", "http://localhost:8000/v1"];
const missing = [...new Set([...said, ...marked])].filter((s) => !(s in FR) && !same.includes(s));
assert.deepEqual(missing, [], "texts without French");
console.log("ok");
