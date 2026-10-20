// node ui/themes.test.js — the four themes, selectable in the settings:
// Fennec Sand (dark), Fennec Sand Light, Fennec Slate, the first fennec's
// palette, and Fennec Orange, the v4 design's. Each theme is a single
// palette of its own; there is no appearance mode. Display-only preferences
// kept in localStorage; unknown ids fall back to Sable.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
// `t` in English, as app.js shows it by default.
globalThis.t = (s, ...a) => s.replace(/\{(\d+)\}/g, (_, i) => String(a[i] ?? ""));
globalThis.lang = () => "en";
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n\\}\\n`))[0];

const style = { colorScheme: "" };
style.setProperty = (k, v) => { style[k] = v; };
style.removeProperty = (k) => { delete style[k]; };
global.document = { documentElement: { style } };
const store = {};
global.localStorage = {
  getItem: (k) => (k in store ? store[k] : null),
  setItem: (k, v) => { store[k] = String(v); },
};

eval(src.match(/const THEME_KEY = [^\n]*\n/)[0]
  + src.match(/const SABLE_LIGHT = \{[\s\S]*?\n\};/)[0]
  + src.match(/const SABLE_DARK = \{[\s\S]*?\n\};/)[0]
  + src.match(/const THEMES = \[[\s\S]*?\n\];/)[0]
  + grab("isLight") + grab("applyTheme") + grab("loadTheme") + grab("saveTheme")
  + "globalThis.THEMES = THEMES; globalThis.SABLE_LIGHT = SABLE_LIGHT; globalThis.SABLE_DARK = SABLE_DARK; globalThis.applyTheme = applyTheme; globalThis.loadTheme = loadTheme; globalThis.saveTheme = saveTheme;");

assert.equal(THEMES.length, 4, "quatre thèmes");
assert.equal(THEMES[0].id, "sable", "Sable d'abord : c'est le défaut");
assert.equal(THEMES[0].name, "Fennec Sand");
assert.equal(THEMES[1].id, "sable-clair", "le mode clair est un thème à part entière");
assert.equal(THEMES[1].name, "Fennec Sand Light");
assert.equal(THEMES[2].id, "slate");
assert.equal(THEMES[2].name, "Fennec Slate");
assert.equal(THEMES[3].id, "orange");
assert.equal(THEMES[3].name, "Fennec Orange");

// Each theme is one palette of eleven tokens.
for (const t of THEMES) assert.equal(Object.keys(t.palette).length, 11, t.id + " : les onze tokens");

// Sable carries the CSS's own dark palette; Sable Clair, the light one, verbatim.
assert.equal(THEMES[0].palette["--bg"], "#171716");
assert.equal(THEMES[0].palette["--accent"], "#e08050");
assert.equal(THEMES[1].palette["--bg"], "#f6f5f2");
assert.equal(THEMES[1].palette["--card"], "#fff");
assert.equal(THEMES[1].palette["--accent"], "#c4622d");

// Slate carries the first fennec's default palette on the lead's tokens.
const t = THEMES[2].palette;
assert.equal(t["--bg"], "#191d24");
assert.equal(t["--card"], "#1f242d");
assert.equal(t["--text"], "#dde2e9");
assert.equal(t["--muted"], "#98a0ad");
assert.equal(t["--line"], "#353c49");
assert.equal(t["--soft"], "#272d38");
assert.equal(t["--accent"], "#e5a55c");
assert.equal(t["--ok"], "#62b183");
assert.equal(t["--warn"], "#d5983f");
assert.equal(t["--bad"], "#dd6b64");
assert.equal(t["--info"], "#6fa0df");

// Orange carries the v4 design's "fennec orange" on its near-black navy.
const o = THEMES[3].palette;
assert.equal(o["--bg"], "#0b0e14");
assert.equal(o["--card"], "#10141c");
assert.equal(o["--text"], "#e7eaf1");
assert.equal(o["--muted"], "#79839a");
assert.equal(o["--line"], "#232b3a");
assert.equal(o["--soft"], "#1b2230");
assert.equal(o["--accent"], "#f0883e", "l'orange fennec du design v4");
assert.equal(o["--ok"], "#4cc38a");
assert.equal(o["--warn"], "#e0a83e");
assert.equal(o["--bad"], "#e5534b");
assert.equal(o["--info"], "#5aa9e6");

// Applying a theme writes its palette on the root; native widgets follow it.
saveTheme("orange");
applyTheme();
assert.equal(style["--bg"], "#0b0e14");
assert.equal(style["--accent"], "#f0883e");
assert.equal(style.colorScheme, "dark");

saveTheme("sable-clair");
applyTheme();
assert.equal(style["--bg"], "#f6f5f2", "Sable Clair écrit sa palette");
assert.equal(style["--accent"], "#c4622d");
assert.equal(style.colorScheme, "light", "widgets natifs : fond clair");

saveTheme("sable");
applyTheme();
assert.equal(style["--bg"], "#171716", "Sable reste sombre");
assert.equal(style.colorScheme, "dark");

saveTheme("slate");
applyTheme();
assert.equal(style["--bg"], "#191d24");
assert.equal(style.colorScheme, "dark");

assert.equal(isLight("#f6f5f2"), true, "le sable clair est clair");
assert.equal(isLight("#191d24"), false, "le slate est sombre");

// The choice persists; a missing or unknown id falls back to Sable.
saveTheme("slate");
assert.equal(store["fennec.theme"], "slate");
assert.equal(loadTheme(), "slate");
delete store["fennec.theme"];
assert.equal(loadTheme(), "sable", "aucun choix : Sable");
store["fennec.theme"] = "inconnu";
assert.equal(loadTheme(), "sable", "id inconnu : Sable");
console.log("ok");
