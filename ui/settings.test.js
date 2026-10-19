// node ui/settings.test.js — the settings page, one tab per concern:
// general, appearance, skills. General open by default; a skill row ends in a
// switch (on : enabled, off : not), not a text button.
const assert = require("assert");
const html = require("fs").readFileSync(__dirname + "/index.html", "utf8");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");

// The tab bar: three tabs, one panel each, general first and open.
assert.match(html, /<div class="tabs" role="tablist" id="settings-tabs">/, "la barre d'onglets");
for (const [label, panel, open] of [["Général", "settings-general", true], ["Apparence", "settings-appearance", false], ["Skills", "settings-skills", false]]) {
  assert.match(html, new RegExp(`<button type="button" role="tab" aria-selected="${open}" data-panel="${panel}">${label}</button>`), label + " : son onglet");
  assert.match(html, new RegExp(`<div id="${panel}"${open ? "" : " hidden"}>`), label + " : son panneau" + (open ? ", ouvert" : ", fermé"));
}
// Each panel keeps its content.
assert.match(html, /<div id="settings-general">\s*<form id="settings-form">/, "le formulaire dans Général");
assert.match(html, /<div id="settings-appearance" hidden>\s*<ul id="theme-picker">/, "les thèmes dans Apparence");
assert.match(html, /<div id="settings-skills" hidden>[\s\S]*<input id="skill-search"[\s\S]*<ul id="skills-list">/, "la recherche et la liste dans Skills");
assert.doesNotMatch(html.match(/<section id="settings" hidden>[\s\S]*?\n  <\/section>/)[0], /<h3>/, "plus de titres de section : les onglets les remplacent");

// The switch in the CSS, driven by aria-checked.
assert.match(html, /\.switch \{/, "le style du switch");
assert.match(html, /\.switch\[aria-checked=true\] \{ background: var\(--ok\); \}/, "allumé : l'ok");
assert.match(html, /\.switch\[aria-checked=true\]::after \{ transform: translateX\(14px\); \}/, "allumé : le bouton glisse");

// The skill row: a switch, not a text button; state read from chosen.
const draw = src.match(/function drawSkillList\(\) \{[\s\S]*?\n\}/)[0];
assert.doesNotMatch(draw, /Ajouter|Retirer/, "plus de boutons Ajouter/Retirer");
assert.match(draw, /className: "switch"/, "le switch");
assert.match(src, /b\.setAttribute\("role", "switch"\)/, "role switch");
assert.match(src, /b\.setAttribute\("aria-checked", String\(s\.chosen\)\)/, "l'état : choisi = allumé");

// The theme picker: one tile per theme, its palette as a preview, the chosen
// one pressed; no more radio rows.
assert.match(html, /#theme-picker \{[^}]*grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/, "les tuiles, deux colonnes");
assert.match(html, /\.theme\[aria-pressed=true\] \{ border-color: var\(--accent\); \}/, "la tuile choisie");
const picker = src.match(/function buildThemePicker\(\) \{[\s\S]*?\n\}/)[0];
assert.match(picker, /className: "theme"/, "une tuile par thème");
assert.match(picker, /setAttribute\("aria-pressed", String\(t\.id === loadTheme\(\)\)\)/, "l'état initial : le thème chargé");
assert.doesNotMatch(picker, /type: "radio"/, "plus de radios");

// The font size: a number field in pixels, not a slider; old zoom values migrate.
assert.match(html, /<input id="font-scale" type="number" min="12" max="21" step="1"/, "un champ nombre, bornes 12–21");
assert.doesNotMatch(html, /type="range"/, "plus de slider");
const scale = src.match(/const FONT_KEY[\s\S]*?\napplyFontScale\(\);\n/)[0];
const store2 = {};
global.document = { documentElement: { style: {} } };
global.localStorage = {
  getItem: (k) => (k in store2 ? store2[k] : null),
  setItem: (k, v) => { store2[k] = String(v); },
};
eval(scale + "globalThis.FONT_KEY = FONT_KEY; globalThis.loadFontScale = loadFontScale;");
assert.equal(loadFontScale(), 15, "aucun choix : 15");
store2[FONT_KEY] = "1.2";
assert.equal(loadFontScale(), 18, "l'ancien zoom 1.2 : 18 px");
store2[FONT_KEY] = "17";
assert.equal(loadFontScale(), 17, "un px reste un px");
store2[FONT_KEY] = "42";
assert.equal(loadFontScale(), 15, "hors bornes : 15");

// The tab wiring: a click selects its tab and shows only its panel.
const tabs = ["settings-general", "settings-appearance", "settings-rules", "settings-skills", "settings-clean"].map((panel) => ({
  dataset: { panel }, setAttribute(k, v) { this[k] = v; }, onclick: null,
}));
const panels = {};
for (const id of tabs.map((t) => t.dataset.panel)) panels[id] = { hidden: id !== "settings-general" };
global.document = {
  documentElement: { style: {} },
  querySelectorAll: (q) => (q.includes("settings-tabs") ? tabs : []),
  getElementById: (id) => panels[id],
};
const $ = (id) => panels[id];
eval(src.match(/\n\/\/ The settings panels[\s\S]*?\n\}\);\n/)[0]);
tabs[3].onclick();
assert.equal(tabs[3]["aria-selected"], "true", "l'onglet Skills est sélectionné");
assert.equal(tabs[0]["aria-selected"], "false", "Général n'est plus sélectionné");
assert.deepEqual(tabs.map((t) => t["aria-selected"]), ["false", "false", "false", "true", "false"]);
assert.deepEqual(Object.fromEntries(Object.entries(panels).map(([k, v]) => [k, v.hidden])),
  { "settings-general": true, "settings-appearance": true, "settings-rules": true, "settings-skills": false, "settings-clean": true }, "seul le panneau Skills est visible");
tabs[0].onclick();
assert.equal(panels["settings-general"].hidden, false, "revenir à Général");
assert.equal(panels["settings-skills"].hidden, true, "Skills refermé");
// Opening "Nettoyage" loads what is left behind.
let loaded = 0;
global.showLeftovers = () => loaded++;
tabs[4].onclick();
assert.equal(panels["settings-clean"].hidden, false, "Nettoyage visible");
assert.equal(loaded, 1, "la liste est chargée à l'ouverture");
// Opening "Règles" reads the rules for every project, no project named.
let read;
global.changeRules = (box, path, scope) => (read = [box, path, scope]);
tabs[2].onclick();
assert.deepEqual(read, [panels["settings-rules"], "", "user"], "les règles de l'utilisateur sont lues à l'ouverture");
console.log("ok");
