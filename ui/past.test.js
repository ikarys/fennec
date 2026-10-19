// node ui/past.test.js — a finished request read in full is a plain block:
// its title is a line, not a fold; the list's toggle does not come along.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];
const arrow = (name) => src.match(new RegExp(`const ${name} = [^\\n]*\\n`))[0];

class Node {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.textContent = "";
    this.className = "";
    this.dataset = {};
    this.style = { setProperty() {} };
    this.classList = { add() {} };
  }
  append(...cs) { this.children.push(...cs); }
  querySelector() { return new Node("div"); }
  querySelectorAll() { return []; }
  replaceWith(o) { this.replacedBy = o; }
}
global.document = {
  createElement: (t) => new Node(t),
  getElementById: () => new Node("div"),
};
global.fetch = async () => ({ ok: true, json: async () => ({}) });

eval(src.match(/const el = \(tag, props = \{\}, \.\.\.children\) => \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICONS = \{[\s\S]*?\n\};/)[0]
  + src.match(/const ICON_OF = \[\[[\s\S]*?\];\n/)[0]
  + grab("setIcon") + grab("iconize")
  + arrow("errorOf")
  + src.match(/async function api\([\s\S]*?\n}\n/)[0]
  + grab("action") + grab("forgetButton") + grab("opener")
  + grab("journalButton") + grab("changesButton")
  + src.match(/async function changes\([\s\S]*?\n}\n/)[0] + grab("diffView")
  + grab("rich") + grab("inline") + grab("md") + grab("focus") + grab("firstLine")
  + grab("conversation") + grab("doneWhen") + grab("work") + grab("shots")
  + grab("line") + grab("headline") + grab("pastState") + grab("pill")
  + src.match(/const METRIC_TIP = \{[\s\S]*?\n\};\n/)[0]
  + grab("metrics") + grab("metric")
  + grab("timeline")
  + src.match(/const STAGE_TIP = \{[\s\S]*?\};\n/)[0]
  + arrow("roleTag") + grab("duration") + grab("facts") + arrow("lead") + grab("details")
  + src.match(/const STEPS = \[[\s\S]*?\]\];\n/)[0] + grab("stages") + grab("ended") + grab("past")
  + arrow("stageRank") + arrow("number") + arrow("compact") + arrow("percent")
  + arrow("tps") + arrow("stageOf") + arrow("stageName")
  + "globalThis.past = past; globalThis.roleTag = roleTag;");

const box = past({ id: "p1", title: "rend l'interface plus jolie", kind: "dropped" });
assert.equal(box.tag, "div", "plus un <details> : rien à replier en vue détail");
assert.ok(!box.children.some((c) => c.className === "line"), "plus de ligne de titre : l'en-tête la dit une fois");
assert.ok(!box.children.some((c) => c.tag === "summary"), "aucun summary");

// Every element of the cost block carries a short explanatory tooltip:
// the metric cards, the bar's segments, the role chips (brief if kept).
assert.equal(roleTag("Dev senior", "solve", "Code, puis lance les tests.").title, "Code, puis lance les tests.");
assert.equal(roleTag("QA", "prove").title, "Preuve : le résultat est vérifié avec les tests et les commandes du rôle.");
assert.equal(roleTag("Chef de projet", "frame").title, "Cadrage : les tests qui prouveront la fin et le périmètre, posés avant de coder.");
const box2 = past({ id: "p2", title: "t", kind: "dropped", stats: { turns: 1, ms: 1000, sent: 10, received: 5, cached: 2, thinking: 1, stages: { "Travail": { ms: 1000, roles: ["Dev senior"] } } } });
const costBlock = box2.children.find((c) => c.className === "facts");
assert.deepEqual(costBlock.children.filter((c) => c.tag === "h3").map((c) => c.textContent),
  ["Répartition du temps", "Équipe", "L'isolement"], "un titre de section par type, le coût porté par l'en-tête");
assert.ok(costBlock.children.find((c) => c.textContent === "Équipe").title.includes("page Équipe"), "le titre Équipe renvoie au concept, défini dans la page Équipe");
const cards = costBlock.children.find((c) => c.className === "metrics").children;
assert.ok(cards.every((c) => c.title), "chaque figure de coût a un tooltip explicatif");
assert.equal(cards.find((c) => c.children.some((k) => k.textContent === "Appels")).title, "Nombre d'appels faits au modèle pour cette tâche.");
const segs = costBlock.children.find((c) => c.className === "timeline").children;
assert.ok(segs.length && segs.every((c) => c.title), "chaque segment explique son étape en une phrase");
assert.equal(segs[0].title, "Réalisation : le travail est écrit et exécuté, essai après essai.");

console.log("OK");
