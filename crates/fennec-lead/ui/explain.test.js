// node ui/explain.test.js — checks reasons are put in the human's words.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// In French through fr.js: the reasons and their translation together.
eval(require("fs").readFileSync(__dirname + "/fr.js", "utf8").replace("const FR", "globalThis.FR"));
globalThis.lang = () => "fr";
eval(src.match(/function t\([\s\S]*?\n}\n/)[0].replace("function t(", "globalThis.t = function ("));
const firstLine = (s) => (s ?? "").split("\n")[0];
const grab = (name) => src.match(new RegExp(`function ${name}[\\s\\S]*?\\n}\\n`))[0];
eval(grab("unreachable") + grab("explain"));

assert.match(explain("timeout: global"), /pas à temps/);
assert.match(explain("io: Connection refused (os error 111)"), /rien ne répond/);
assert.match(explain("http status: 401"), /refuse la clé/);
assert.match(explain("http status: 500"), /erreur 500/);
assert.match(explain("http status: 404"), /ne connaît pas cette adresse/);
assert.match(explain("http status: 404: model 'big' not found"), /ne sert pas le modèle « big »/);
assert.match(explain("http status: 500: out of memory"), /erreur 500/);
assert.match(explain("host not found"), /n'existe pas/);
assert.match(
  explain("no model: none is set and http://x:1/v1/models did not answer (timeout: global)"),
  /aucun modèle à http:\/\/x:1\/v1 \(le serveur ne répond pas à temps\)/,
);
assert.match(
  explain("git rev-parse --path-format=absolute --git-common-dir: fatal: not a git repository (or any parent up to mount point /)"),
  /n'est plus un dépôt git/,
);
assert.match(explain("the project folder has uncommitted changes"), /changements non commités/);
assert.match(explain("the project folder already holds work in progress (world 1a)"), /autre tâche/);
// A test's own output is not the server's.
assert.equal(explain("tests failed: case ... timeout: 5s"), "tests failed: case ... timeout: 5s");
console.log("ok");
