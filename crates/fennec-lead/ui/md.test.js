// node ui/md.test.js — the model's Markdown becomes blocks, not one wall of text.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";
const grab = (name) => src.match(new RegExp(`function ${name}[\\s\\S]*?\\n}\\n`))[0];
const el = (tag, props = {}, ...children) => {
  const keep = (c) => c !== null && c !== undefined && c !== false;
  const node = { tag, props, children: children.filter(keep) };
  node.append = (...cs) => node.children.push(...cs.filter(keep));
  return node;
};
eval(grab("isImage") + grab("inline") + grab("md"));

const tags = (blocks) => blocks.map((b) => b.tag);
const deep = (x, tag) => {
  const nodes = Array.isArray(x) ? x : [x];
  const kids = nodes.flatMap((n) => (n && typeof n === "object" ? n.children ?? [] : []));
  return [...nodes, ...kids.flatMap((c) => (typeof c === "object" ? deep(c, tag) : []))]
    .filter((n) => n && typeof n === "object" && n.tag === tag);
};

// A plain line stays a paragraph.
assert.deepStrictEqual(tags(md("Bonjour")), ["p"]);

// Bold and code survive.
let blocks = md("C'est un **monorepo** en `TypeScript`.");
assert.strictEqual(deep(blocks, "b").length, 1);
assert.strictEqual(deep(blocks, "code").length, 1);

// Headings become headings.
assert.deepStrictEqual(tags(md("## Outils en place")), ["h2"]);
assert.deepStrictEqual(tags(md("# Gros")), ["h1"]);

// A table becomes a table, not a row of pipes.
blocks = md("| Script | Rôle |\n|---|---|\n| `npm test` | Vitest |");
assert.deepStrictEqual(tags(blocks), ["table"]);
assert.strictEqual(blocks[0].props.className, "grid");
assert.strictEqual(deep(blocks, "th").length, 2);
assert.strictEqual(deep(blocks, "td").length, 2);
assert.strictEqual(deep(blocks, "code").length, 1);

// Nested lists keep their levels.
blocks = md("- **economy** — ressources\n  - sous-item\n- deux");
assert.deepStrictEqual(tags(blocks), ["ul"]);
const ul = blocks[0];
assert.strictEqual(ul.children.length, 2);
assert.strictEqual(deep(ul.children[0], "ul").length, 1);
assert.strictEqual(deep(ul.children[1], "ul").length, 0);

// Numbered lists.
assert.deepStrictEqual(tags(md("1. un\n2. deux")), ["ol"]);

// Fenced code keeps its text.
blocks = md("Avant :\n```\nfn main() {}\n```\nAprès.");
assert.ok(tags(blocks).includes("pre"));
assert.ok(deep(blocks, "pre")[0].props.textContent.includes("fn main()"));
assert.strictEqual(deep(blocks, "p").length, 2);

// Blockquote and rule.
assert.deepStrictEqual(tags(md("> citation")), ["blockquote"]);
assert.deepStrictEqual(tags(md("---")), ["hr"]);

// Links open outside the app.
blocks = md("Voir [la doc](https://exemple.fr).");
const link = deep(blocks, "a")[0];
assert.strictEqual(link.props.href, "https://exemple.fr");
assert.strictEqual(link.props.target, "_blank");

// A link that would run script stays text: the model writes these links.
blocks = md("[clic](javascript:alert(1))");
assert.strictEqual(deep(blocks, "a").length, 0);
assert.strictEqual(blocks[0].children.join(""), "[clic](javascript:alert(1))");

// Empty text: no block at all.
assert.deepStrictEqual(md(""), []);
console.log("ok");
